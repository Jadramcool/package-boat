//! 局域网地址枚举：生成对局域网浏览器友好的访问 URL 列表。

use std::net::IpAddr;

/// 生成访问 URL。绑定到具体地址时只返回该地址；绑定 `0.0.0.0`/`::` 时枚举网卡：
/// 优先私网 IPv4，按 IP 排序，去重；无可用地址时回退 `127.0.0.1`。
pub fn local_urls(host: &str, port: u16) -> Vec<String> {
    if !host.is_empty() && host != "0.0.0.0" && host != "::" {
        return vec![format!("http://{host}:{port}")];
    }

    let mut addresses: Vec<(IpAddr, bool)> = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            if interface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(ifv4) = interface.addr {
                let ip = ifv4.ip;
                if !ip.is_unspecified() && !ip.is_loopback() {
                    addresses.push((IpAddr::V4(ip), ip.is_private()));
                }
            }
        }
    }

    // 私网优先，其次按 IP 字典序
    addresses.sort_by(|a, b| {
        if a.1 != b.1 {
            return b.1.cmp(&a.1); // private 优先
        }
        a.0.to_string().cmp(&b.0.to_string())
    });

    let mut urls: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<IpAddr> = std::collections::HashSet::new();
    for (ip, _) in addresses {
        if seen.insert(ip) {
            urls.push(format!("http://{ip}:{port}"));
        }
    }
    if urls.is_empty() {
        urls.push(format!("http://127.0.0.1:{port}"));
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_host_returns_single_url() {
        let urls = local_urls("192.168.1.5", 8080);
        assert_eq!(urls, vec!["http://192.168.1.5:8080"]);
    }

    #[test]
    fn wildcard_host_never_empty() {
        let urls = local_urls("0.0.0.0", 0);
        assert!(!urls.is_empty());
        assert!(urls.iter().all(|url| url.starts_with("http://")));
    }
}
