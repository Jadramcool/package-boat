//! 配对码认证：六位随机配对码、常量时间比较、IP 限速、12 小时会话令牌。

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use std::collections::HashMap;
use std::sync::Mutex;
use subtle::ConstantTimeEq;

/// 会话 Cookie 名称，与 Go 版一致。
pub const SESSION_COOKIE_NAME: &str = "packetboat_session";

const CODE_LENGTH: usize = 6;
const SESSION_TTL: Duration = Duration::hours(12);
const ATTEMPT_WINDOW: Duration = Duration::minutes(10);
const MAX_ATTEMPTS: i32 = 8;
const BLOCK_DURATION: Duration = Duration::minutes(2);

#[derive(Default)]
struct AttemptState {
    count: i32,
    window_start: Option<DateTime<Utc>>,
    blocked_to: Option<DateTime<Utc>>,
}

struct AuthInner {
    sessions: HashMap<String, DateTime<Utc>>,
    attempts: HashMap<String, AttemptState>,
}

/// 认证管理器：持有当前配对码与会话表。
pub struct AuthManager {
    code: String,
    inner: Mutex<AuthInner>,
}

impl AuthManager {
    pub fn new() -> Self {
        AuthManager {
            code: random_digits(CODE_LENGTH),
            inner: Mutex::new(AuthInner {
                sessions: HashMap::new(),
                attempts: HashMap::new(),
            }),
        }
    }

    /// 当前配对码。
    pub fn code(&self) -> &str {
        &self.code
    }

    /// 尝试配对。成功时返回（令牌, 过期时间），失败返回错误信息。
    pub fn pair(&self, ip: &str, supplied: &str) -> Result<(String, DateTime<Utc>), String> {
        let now = Utc::now();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        // 清理过期尝试记录，防止 attempts map 无限增长
        inner.attempts.retain(|_, state| {
            state.window_start.is_none()
                || now <= state.window_start.unwrap() + ATTEMPT_WINDOW + BLOCK_DURATION
        });
        let state = inner.attempts.entry(ip.to_string()).or_default();

        if let Some(blocked_to) = state.blocked_to {
            if now < blocked_to {
                return Err("尝试次数过多，请稍后再试".to_string());
            }
        }
        if state.window_start.is_none() || now - state.window_start.unwrap() > ATTEMPT_WINDOW {
            state.window_start = Some(now);
        }

        let valid = supplied.len() == self.code.len()
            && supplied.as_bytes().ct_eq(self.code.as_bytes()).into();
        if !valid {
            state.count += 1;
            if state.count >= MAX_ATTEMPTS {
                state.blocked_to = Some(now + BLOCK_DURATION);
                state.count = 0;
            }
            return Err("配对码不正确".to_string());
        }

        inner.attempts.remove(ip);
        let mut token_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut token_bytes);
        let token = URL_SAFE_NO_PAD.encode(token_bytes);
        let expires = now + SESSION_TTL;
        inner.sessions.insert(token.clone(), expires);
        remove_expired_locked(&mut inner.sessions, now);
        Ok((token, expires))
    }

    /// 校验会话令牌是否有效；过期或未知令牌返回 false 并清理。
    pub fn authenticated(&self, token: Option<&str>) -> bool {
        let token = match token {
            Some(token) if !token.is_empty() => token,
            _ => return false,
        };
        let now = Utc::now();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        match inner.sessions.get(token) {
            Some(expires) if now <= *expires => true,
            Some(_) => {
                inner.sessions.remove(token);
                false
            }
            None => false,
        }
    }

    /// 注销：删除对应会话。
    pub fn sign_out(&self, token: Option<&str>) {
        if let Some(token) = token {
            let mut inner = self.inner.lock().expect("auth lock poisoned");
            inner.sessions.remove(token);
        }
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

fn remove_expired_locked(sessions: &mut HashMap<String, DateTime<Utc>>, now: DateTime<Utc>) {
    sessions.retain(|_, expires| now <= *expires);
}

/// 生成指定长度的随机数字字符串（模 10 有轻微偏差，与 Go 版一致）。
fn random_digits(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::rng().fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|value| char::from(b'0' + value % 10))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_six_digits() {
        let manager = AuthManager::new();
        assert_eq!(manager.code().len(), 6);
        assert!(manager.code().chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn pair_and_authenticate_roundtrip() {
        let manager = AuthManager::new();
        assert!(!manager.authenticated(None));
        let (token, _) = manager.pair("127.0.0.1", manager.code()).unwrap();
        assert!(manager.authenticated(Some(&token)));
        manager.sign_out(Some(&token));
        assert!(!manager.authenticated(Some(&token)));
    }

    #[test]
    fn wrong_code_rejected_and_rate_limited() {
        let manager = AuthManager::new();
        for _ in 0..7 {
            assert!(manager.pair("10.0.0.1", "000000").is_err());
        }
        // 第 8 次失败触发 2 分钟封禁
        assert!(manager.pair("10.0.0.1", "000000").is_err());
        let err = manager.pair("10.0.0.1", manager.code()).unwrap_err();
        assert!(err.contains("过多"));
        // 其它 IP 不受影响
        assert!(manager.pair("10.0.0.2", manager.code()).is_ok());
    }

    #[test]
    fn correct_code_clears_attempts() {
        let manager = AuthManager::new();
        for _ in 0..5 {
            let _ = manager.pair("10.0.0.3", "111111");
        }
        assert!(manager.pair("10.0.0.3", manager.code()).is_ok());
        assert!(manager.pair("10.0.0.3", manager.code()).is_ok());
    }
}
