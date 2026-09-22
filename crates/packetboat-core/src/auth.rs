//! 配对码认证：六位随机配对码、常量时间比较、IP 限速、12 小时会话令牌。
//! 会话与免配对访问会登记设备元数据；SSE 长连接用于判定「当前仍连接」。

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use subtle::ConstantTimeEq;

/// 会话 Cookie 名称，与 Go 版一致。
pub const SESSION_COOKIE_NAME: &str = "packetboat_session";

const CODE_LENGTH: usize = 6;
const SESSION_TTL: Duration = Duration::hours(12);
const ATTEMPT_WINDOW: Duration = Duration::minutes(10);
const MAX_ATTEMPTS: i32 = 8;
const BLOCK_DURATION: Duration = Duration::minutes(2);
/// 无长连接时的最近活动窗口：超过后从「已连接设备」列表移除。
const ONLINE_WINDOW: Duration = Duration::seconds(20);
/// 免配对 peer 内部保留时长。
const PEER_TTL: Duration = Duration::hours(24);

/// 设备接入方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum DeviceAuthMode {
    /// 使用六位配对码建立的会话。
    Paired,
    /// 免配对模式下的匿名访问。
    Anonymous,
}

/// 桌面端展示的已连接设备。
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct ConnectedDevice {
    /// 展示用短 ID（不暴露完整会话令牌）。
    pub id: String,
    pub ip: String,
    pub user_agent: String,
    pub device_label: String,
    pub auth_mode: DeviceAuthMode,
    pub paired_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    /// 会话过期时间；免配对 peer 为最近活动 + PEER_TTL。
    pub expires_at: DateTime<Utc>,
    /// 当前仍算「已连接」（本列表只返回 true 的条目）。
    pub online: bool,
    /// 是否仍有活跃长连接（SSE 等）。
    pub connected: bool,
}

/// 长连接租约：Drop（页面关闭 / SSE 断开）时自动减少连接计数。
pub struct DeviceConnection {
    auth: Arc<AuthManager>,
    key: String,
}

impl DeviceConnection {
    pub fn new(auth: Arc<AuthManager>, key: String) -> Self {
        DeviceConnection { auth, key }
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for DeviceConnection {
    fn drop(&mut self) {
        self.auth.end_connection(&self.key);
    }
}

#[derive(Default)]
struct AttemptState {
    count: i32,
    window_start: Option<DateTime<Utc>>,
    blocked_to: Option<DateTime<Utc>>,
}

struct SessionRecord {
    expires: DateTime<Utc>,
    ip: String,
    user_agent: String,
    device_label: String,
    paired_at: DateTime<Utc>,
    last_seen: DateTime<Utc>,
}

struct PeerRecord {
    ip: String,
    user_agent: String,
    /// 用户自定义设备名；为空时按 UA 推断展示。
    device_label: String,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
}

struct AuthInner {
    sessions: HashMap<String, SessionRecord>,
    /// 免配对访问设备：`ip|user_agent` → 记录。
    peers: HashMap<String, PeerRecord>,
    attempts: HashMap<String, AttemptState>,
    /// 活跃长连接计数：`paired:{token}` / `peer:{ip}|{ua}` → 次数。
    connections: HashMap<String, u32>,
}

/// 认证管理器：持有当前配对码、会话表与设备活动记录。
pub struct AuthManager {
    code: std::sync::RwLock<String>,
    inner: Mutex<AuthInner>,
}

impl AuthManager {
    pub fn new() -> Self {
        AuthManager {
            code: std::sync::RwLock::new(random_digits(CODE_LENGTH)),
            inner: Mutex::new(AuthInner {
                sessions: HashMap::new(),
                peers: HashMap::new(),
                attempts: HashMap::new(),
                connections: HashMap::new(),
            }),
        }
    }

    /// 当前配对码。
    pub fn code(&self) -> String {
        self.code.read().expect("auth code lock poisoned").clone()
    }

    /// 手动刷新配对码：生成新六位码并返回。已有会话不受影响；失败尝试计数清零。
    pub fn rotate_code(&self) -> String {
        let next = random_digits(CODE_LENGTH);
        {
            let mut code = self.code.write().expect("auth code lock poisoned");
            *code = next.clone();
        }
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        inner.attempts.clear();
        next
    }

    /// 尝试配对。成功时返回（令牌, 过期时间），失败返回错误信息。
    pub fn pair(
        &self,
        ip: &str,
        user_agent: &str,
        device_label: &str,
        supplied: &str,
    ) -> Result<(String, DateTime<Utc>), String> {
        let now = Utc::now();
        let current = self.code();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        prune_locked(&mut inner, now);
        let state = inner.attempts.entry(ip.to_string()).or_default();

        if let Some(blocked_to) = state.blocked_to {
            if now < blocked_to {
                return Err("尝试次数过多，请稍后再试".to_string());
            }
        }
        if state.window_start.is_none() || now - state.window_start.unwrap() > ATTEMPT_WINDOW {
            state.window_start = Some(now);
        }

        let valid =
            supplied.len() == current.len() && supplied.as_bytes().ct_eq(current.as_bytes()).into();
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
        let label = normalize_label(device_label, user_agent);
        inner.sessions.insert(
            token.clone(),
            SessionRecord {
                expires,
                ip: ip.to_string(),
                user_agent: user_agent.to_string(),
                device_label: label,
                paired_at: now,
                last_seen: now,
            },
        );
        Ok((token, expires))
    }

    /// 校验会话令牌是否有效；过期或未知令牌返回 false 并清理。
    /// 有效时刷新 `last_seen`，并可选更新 IP / UA / 自定义设备名。
    pub fn authenticated(
        &self,
        token: Option<&str>,
        ip: Option<&str>,
        user_agent: Option<&str>,
        _device_label: Option<&str>,
    ) -> bool {
        let token = match token {
            Some(token) if !token.is_empty() => token,
            _ => return false,
        };
        let now = Utc::now();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        match inner.sessions.get_mut(token) {
            Some(record) if now <= record.expires => {
                record.last_seen = now;
                if let Some(ip) = ip.filter(|value| !value.is_empty() && *value != "unknown") {
                    record.ip = ip.to_string();
                }
                if let Some(ua) = user_agent.filter(|value| !value.is_empty()) {
                    record.user_agent = ua.to_string();
                    // 设备名始终由 UA 推导（如 Android 机型），不使用自定义名
                    record.device_label = normalize_label("", ua);
                }
                true
            }
            Some(_) => {
                inner.sessions.remove(token);
                false
            }
            None => false,
        }
    }

    /// 免配对模式：按 IP + User-Agent 登记/刷新访问设备。
    pub fn touch_peer(&self, ip: &str, user_agent: &str, device_label: Option<&str>) {
        let now = Utc::now();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        prune_locked(&mut inner, now);
        touch_peer_locked(&mut inner, ip, user_agent, device_label, now);
    }

    /// 登记长连接（SSE 等），返回连接 key。
    pub fn begin_connection(
        &self,
        token: Option<&str>,
        ip: &str,
        user_agent: &str,
        device_label: Option<&str>,
    ) -> String {
        let now = Utc::now();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        prune_locked(&mut inner, now);

        let key = match token {
            Some(token) if !token.is_empty() => {
                let session_ok = inner
                    .sessions
                    .get(token)
                    .map(|record| now <= record.expires)
                    .unwrap_or(false);
                if session_ok {
                    if let Some(record) = inner.sessions.get_mut(token) {
                        record.last_seen = now;
                        if !ip.is_empty() && ip != "unknown" {
                            record.ip = ip.to_string();
                        }
                        if !user_agent.is_empty() {
                            record.user_agent = user_agent.to_string();
                        }
                        if let Some(label) = device_label
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            record.device_label = label.to_string();
                        }
                    }
                    format!("paired:{token}")
                } else {
                    inner.sessions.remove(token);
                    touch_peer_locked(&mut inner, ip, user_agent, device_label, now);
                    format!("peer:{}", peer_key(ip, user_agent))
                }
            }
            _ => {
                touch_peer_locked(&mut inner, ip, user_agent, device_label, now);
                format!("peer:{}", peer_key(ip, user_agent))
            }
        };

        *inner.connections.entry(key.clone()).or_insert(0) += 1;
        key
    }

    /// 长连接结束：页面关闭 / SSE 断开时调用。
    pub fn end_connection(&self, key: &str) {
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        if let Some(count) = inner.connections.get_mut(key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                inner.connections.remove(key);
            }
        }
    }

    /// 为已登记的连接 key 创建 Drop 租约。
    pub fn lease_connection(self: &Arc<Self>, key: String) -> DeviceConnection {
        DeviceConnection::new(Arc::clone(self), key)
    }

    /// 注销：删除对应会话及其长连接计数。
    pub fn sign_out(&self, token: Option<&str>) {
        if let Some(token) = token {
            let mut inner = self.inner.lock().expect("auth lock poisoned");
            inner.sessions.remove(token);
            inner.connections.remove(&format!("paired:{token}"));
        }
    }

    /// 当前仍处于连接中的设备列表（关闭后自动移除；按最近活动降序）。
    pub fn devices(&self) -> Vec<ConnectedDevice> {
        let now = Utc::now();
        let mut inner = self.inner.lock().expect("auth lock poisoned");
        prune_locked(&mut inner, now);

        let AuthInner {
            sessions,
            peers,
            connections,
            ..
        } = &*inner;

        let mut devices: Vec<ConnectedDevice> = Vec::new();
        for (token, record) in sessions.iter() {
            let conn_key = format!("paired:{token}");
            let streams = connections.get(&conn_key).copied().unwrap_or(0);
            let recent = now - record.last_seen <= ONLINE_WINDOW;
            if streams == 0 && !recent {
                continue;
            }
            devices.push(ConnectedDevice {
                id: display_id("paired", token),
                ip: record.ip.clone(),
                user_agent: record.user_agent.clone(),
                device_label: record.device_label.clone(),
                auth_mode: DeviceAuthMode::Paired,
                paired_at: record.paired_at,
                last_seen: record.last_seen,
                expires_at: record.expires,
                online: true,
                connected: streams > 0,
            });
        }
        for (key, record) in peers.iter() {
            let conn_key = format!("peer:{key}");
            let streams = connections.get(&conn_key).copied().unwrap_or(0);
            let recent = now - record.last_seen <= ONLINE_WINDOW;
            if streams == 0 && !recent {
                continue;
            }
            let label = if record.device_label.trim().is_empty() {
                normalize_label("", &record.user_agent)
            } else {
                record.device_label.clone()
            };
            devices.push(ConnectedDevice {
                id: display_id("anon", key),
                ip: record.ip.clone(),
                user_agent: record.user_agent.clone(),
                device_label: label,
                auth_mode: DeviceAuthMode::Anonymous,
                paired_at: record.first_seen,
                last_seen: record.last_seen,
                expires_at: record.last_seen + PEER_TTL,
                online: true,
                connected: streams > 0,
            });
        }
        devices.sort_by_key(|device| std::cmp::Reverse(device.last_seen));
        devices
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

fn touch_peer_locked(
    inner: &mut AuthInner,
    ip: &str,
    user_agent: &str,
    _device_label: Option<&str>,
    now: DateTime<Utc>,
) {
    let key = peer_key(ip, user_agent);
    let derived = if user_agent.trim().is_empty() {
        String::new()
    } else {
        normalize_label("", user_agent)
    };
    let entry = inner.peers.entry(key).or_insert_with(|| PeerRecord {
        ip: ip.to_string(),
        user_agent: user_agent.to_string(),
        device_label: derived.clone(),
        first_seen: now,
        last_seen: now,
    });
    entry.last_seen = now;
    if !ip.is_empty() && ip != "unknown" {
        entry.ip = ip.to_string();
    }
    if !user_agent.is_empty() {
        entry.user_agent = user_agent.to_string();
        entry.device_label = derived;
    }
}

fn prune_locked(inner: &mut AuthInner, now: DateTime<Utc>) {
    inner.sessions.retain(|_, record| now <= record.expires);
    inner
        .peers
        .retain(|_, record| now - record.last_seen <= PEER_TTL);
    inner.attempts.retain(|_, state| {
        state.window_start.is_none()
            || now <= state.window_start.unwrap() + ATTEMPT_WINDOW + BLOCK_DURATION
    });

    let stale: Vec<String> = inner
        .connections
        .iter()
        .filter_map(|(key, count)| {
            if *count == 0 {
                return Some(key.clone());
            }
            if let Some(token) = key.strip_prefix("paired:") {
                if !inner.sessions.contains_key(token) {
                    return Some(key.clone());
                }
            } else if let Some(peer) = key.strip_prefix("peer:") {
                if !inner.peers.contains_key(peer) {
                    return Some(key.clone());
                }
            } else {
                return Some(key.clone());
            }
            None
        })
        .collect();
    for key in stale {
        inner.connections.remove(&key);
    }
}

fn peer_key(ip: &str, user_agent: &str) -> String {
    format!("{ip}|{}", user_agent.trim().to_lowercase())
}

fn display_id(prefix: &str, raw: &str) -> String {
    let mut bytes = [0u8; 8];
    // 简单非加密摘要：展示用，避免把完整会话令牌暴露给前端。
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in raw.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = (hash >> (index * 8)) as u8;
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{prefix}-{}", &hex[..8])
}

fn normalize_label(device_label: &str, user_agent: &str) -> String {
    let label = device_label.trim();
    if !label.is_empty() {
        return label.to_string();
    }
    summarize_user_agent(user_agent)
}

/// 从 User-Agent 提取简短可读标签。
/// Android 优先取机型（如 `PJZ110`），否则回退「系统 + 浏览器」。
pub fn summarize_user_agent(user_agent: &str) -> String {
    let ua = user_agent.trim();
    if ua.is_empty() {
        return "未知设备".to_string();
    }
    if let Some(model) = extract_android_model(ua) {
        return model;
    }
    let lower = ua.to_lowercase();
    let os = if lower.contains("android") {
        "Android"
    } else if lower.contains("iphone") || lower.contains("ipad") || lower.contains("ipod") {
        if lower.contains("ipad") {
            "iPad"
        } else if lower.contains("iphone") {
            "iPhone"
        } else {
            "iOS"
        }
    } else if lower.contains("mac os") || lower.contains("macintosh") {
        "macOS"
    } else if lower.contains("windows") {
        "Windows"
    } else if lower.contains("linux") {
        "Linux"
    } else {
        ""
    };
    let browser = browser_from_ua(&lower);
    match (os, browser) {
        ("", "") => {
            let short = if ua.len() > 48 { &ua[..48] } else { ua };
            short.to_string()
        }
        (os, "") => os.to_string(),
        ("", browser) => browser.to_string(),
        (os, browser) => format!("{os} {browser}"),
    }
}

/// 解析 Android UA 中的机型：`(Linux; Android 16; PJZ110 Build/...)` → `PJZ110`。
fn extract_android_model(ua: &str) -> Option<String> {
    let start = ua.find("Android ")? + "Android ".len();
    let rest = &ua[start..];
    let segment = rest.split(';').nth(1)?;
    let model = segment
        .trim()
        .split(" Build/")
        .next()?
        .trim()
        .trim_end_matches(')');
    if model.is_empty()
        || model.eq_ignore_ascii_case("wv")
        || model.eq_ignore_ascii_case("U")
        || model.len() < 2
    {
        return None;
    }
    Some(model.to_string())
}

fn browser_from_ua(lower: &str) -> &'static str {
    if lower.contains("edg/") || lower.contains("edge/") {
        "Edge"
    } else if lower.contains("chrome") && !lower.contains("chromium") {
        "Chrome"
    } else if lower.contains("safari") && !lower.contains("chrome") {
        "Safari"
    } else if lower.contains("firefox") {
        "Firefox"
    } else if lower.contains("curl") {
        "curl"
    } else if lower.contains("okhttp") {
        "OkHttp"
    } else {
        ""
    }
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

    fn pair_ok(manager: &AuthManager, ip: &str) -> String {
        let code = manager.code();
        manager
            .pair(
                ip,
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0) Safari",
                "测试手机",
                &code,
            )
            .unwrap()
            .0
    }

    #[test]
    fn code_has_six_digits() {
        let manager = AuthManager::new();
        assert_eq!(manager.code().len(), 6);
        assert!(manager.code().chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn rotate_code_issues_new_code_and_keeps_sessions() {
        let manager = AuthManager::new();
        let old = manager.code();
        let token = pair_ok(&manager, "127.0.0.1");
        let next = manager.rotate_code();
        assert_eq!(next.len(), 6);
        let next = if next == old {
            manager.rotate_code()
        } else {
            next
        };
        assert_ne!(next, old);
        assert_eq!(manager.code(), next);
        assert!(manager.pair("127.0.0.2", "ua", "", &old).is_err());
        assert!(manager.pair("127.0.0.3", "ua", "", &next).is_ok());
        assert!(
            manager.authenticated(Some(&token), None, None, None),
            "已配对会话应保持有效"
        );
    }

    #[test]
    fn pair_and_authenticate_roundtrip() {
        let manager = AuthManager::new();
        assert!(!manager.authenticated(None, None, None, None));
        let token = pair_ok(&manager, "127.0.0.1");
        assert!(manager.authenticated(Some(&token), None, None, None));
        manager.sign_out(Some(&token));
        assert!(!manager.authenticated(Some(&token), None, None, None));
        assert!(manager.devices().is_empty(), "注销后应从设备列表移除");
    }

    #[test]
    fn wrong_code_rejected_and_rate_limited() {
        let manager = AuthManager::new();
        for _ in 0..7 {
            assert!(manager.pair("10.0.0.1", "ua", "", "000000").is_err());
        }
        assert!(manager.pair("10.0.0.1", "ua", "", "000000").is_err());
        let code = manager.code();
        let err = manager.pair("10.0.0.1", "ua", "", &code).unwrap_err();
        assert!(err.contains("过多"));
        assert!(manager.pair("10.0.0.2", "ua", "", &code).is_ok());
    }

    #[test]
    fn correct_code_clears_attempts() {
        let manager = AuthManager::new();
        for _ in 0..5 {
            let _ = manager.pair("10.0.0.3", "ua", "", "111111");
        }
        let code = manager.code();
        assert!(manager.pair("10.0.0.3", "ua", "", &code).is_ok());
        assert!(manager.pair("10.0.0.3", "ua", "", &code).is_ok());
    }

    #[test]
    fn devices_track_paired_sessions() {
        let manager = AuthManager::new();
        let token = pair_ok(&manager, "192.168.1.20");
        assert!(manager.authenticated(Some(&token), None, None, None));
        let devices = manager.devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].ip, "192.168.1.20");
        assert_eq!(devices[0].auth_mode, DeviceAuthMode::Paired);
        assert_eq!(devices[0].device_label, "测试手机");
        assert!(devices[0].online);
        assert!(devices[0].id.starts_with("paired-"));
        assert!(!devices[0].id.contains(&token));
    }

    #[test]
    fn devices_track_anonymous_peers_when_pairing_disabled() {
        let manager = AuthManager::new();
        manager.touch_peer(
            "10.0.0.9",
            "Mozilla/5.0 (Linux; Android 14; PJZ110 Build/AP1A) Chrome/120.0.0.0 Mobile",
            None,
        );
        manager.touch_peer(
            "10.0.0.9",
            "Mozilla/5.0 (Linux; Android 14; PJZ110 Build/AP1A) Chrome/120.0.0.0 Mobile",
            None,
        );
        let devices = manager.devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].auth_mode, DeviceAuthMode::Anonymous);
        assert_eq!(devices[0].device_label, "PJZ110");
        assert!(devices[0].online);
    }

    #[test]
    fn sse_connection_marks_device_connected_and_drop_removes_stream() {
        let manager = Arc::new(AuthManager::new());
        let token = pair_ok(&manager, "192.168.1.30");
        let key =
            manager.begin_connection(Some(&token), "192.168.1.30", "Safari", Some("客厅 iPad"));
        assert_eq!(key, format!("paired:{token}"));
        let lease = manager.lease_connection(key);

        let devices = manager.devices();
        assert_eq!(devices.len(), 1);
        assert!(devices[0].connected, "SSE 打开时应标记长连接中");

        drop(lease);
        let devices = manager.devices();
        assert_eq!(devices.len(), 1, "刚断开时 last_seen 仍新，暂保留在列表");
        assert!(!devices[0].connected, "SSE 断开后应取消长连接标记");
    }

    #[test]
    fn sign_out_removes_paired_device_immediately() {
        let manager = AuthManager::new();
        let token = pair_ok(&manager, "192.168.1.40");
        let _ = manager.begin_connection(Some(&token), "192.168.1.40", "Safari", None);
        assert_eq!(manager.devices().len(), 1);
        manager.sign_out(Some(&token));
        assert!(manager.devices().is_empty());
    }

    #[test]
    fn user_agent_summaries_are_readable() {
        assert_eq!(summarize_user_agent(""), "未知设备");
        assert_eq!(
            summarize_user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0 Safari/537.36"
            ),
            "Windows Chrome"
        );
        assert_eq!(
            summarize_user_agent(
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Version/17.0 Mobile/15E148 Safari/604.1"
            ),
            "iPhone Safari"
        );
        assert_eq!(
            summarize_user_agent(
                "Mozilla/5.0 (Linux; Android 16; PJZ110 Build/BP2A.250605.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/150.0.7871.181 Mobile Safari/537.36"
            ),
            "PJZ110"
        );
    }
}
