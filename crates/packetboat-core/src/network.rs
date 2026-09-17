//! 局域网地址枚举：生成对局域网浏览器友好的访问 URL 列表。
//!
//! ## 为什么不能只用 `is_private()` 排个序就完事
//!
//! 本机常见叠加多种虚拟网卡（Tailscale、VMware/VirtualBox、Hyper-V 虚拟交换机、
//! 各类点对点代理链路等），它们的地址同样落在 RFC1918 私网段内，但：
//!
//! 1. **点对点链路（/32、/31）** 只有一个对端，广播域里没有第二台设备，
//!    手机永远连不上。典型如 Tailscale 的 `100.x/32`、虚拟网桥的 `/32`。
//! 2. **字符串字典序排序是错的**：`"10.222.222.1" < "10.80.40.166"`（因为 `'2' < '8'`），
//!    会把真实以太网地址排到虚拟链路之后，于是二维码指向一个手机不可达的地址。
//!
//! 因此这里改成三级判定 + 数值排序：
//!
//! - `tier`：可达性分级，`0` = 最可能可达（真实局域网），`2` = 基本不可达（虚拟 / 点对点链路）。
//! - 同 tier 内先比**掩码宽度**（网段越大，同网段设备越多，越可能是真实局域网），
//!   再比 IP 的**数值大小**（`u32`，不是字符串）。
//! - `virtual`：是否被识别为虚拟网卡适配器，UI 用来给地址加提示。

use std::net::IpAddr;

/// 单个候选访问地址及其可达性元信息。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct LocalAddress {
    /// 完整访问 URL，如 `http://192.168.1.8:8080`。
    pub url: String,
    /// 适配器名（Windows 上即「网络连接」显示名），如 `以太网`、`Tailscale`。
    pub interface: String,
    /// 点分十进制 IP（不含端口）。
    pub ip: String,
    /// 前缀长度；无法获取时为 `None`。
    pub prefix_len: Option<u8>,
    /// 该地址是否被识别为虚拟网卡 / 点对点链路。
    pub virtual_link: bool,
    /// 可达性分级，越小越可能被手机访问到。
    /// `0` = 真实局域网，`1` = 存疑，`2` = 虚拟或点对点链路。
    pub tier: u8,
}

impl LocalAddress {
    /// 是否应作为默认主地址（真实局域网且非虚拟网卡）。
    pub fn recommended(&self) -> bool {
        self.tier == 0
    }
}

/// 被识别为「虚拟网卡 / 覆盖网络 / 点对点隧道」的适配器名关键字（小写匹配）。
///
/// 这些适配器上的地址通常无法被同屋的手机直接到达，仅作降级候选。
const VIRTUAL_ADAPTER_KEYWORDS: &[&str] = &[
    "tailscale",
    "zerotier",
    "vether",
    "vethernet",
    "vmware",
    "virtualbox",
    "vbox",
    "hyper-v",
    "vethernet",
    "vgate",
    "nodebabylink",
    "wsl",
    "docker",
    "loopback",
    "tap",
    "tun",
    "npcap",
    "radmin",
    "hamachi",
    "wireguard",
    "openvpn",
    "clash",
    "utun",
    "ppp",
    "bluetooth",
];

/// 判定适配器名是否属于虚拟 / 隧道类网卡。
fn is_virtual_adapter(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    VIRTUAL_ADAPTER_KEYWORDS
        .iter()
        .any(|keyword| lowered.contains(keyword))
}

/// 把 `Ipv4Netmask` 折算成前缀长度（`255.255.252.0` → `22`）。
///
/// 掩码理论上应为连续 1，但虚拟网卡偶尔给出非连续掩码；此时按 1 的个数返回，
/// 只用于排序比较，不做严格校验。
fn prefix_len_of(netmask: std::net::Ipv4Addr) -> u8 {
    u32::from(netmask).count_ones() as u8
}

/// 计算某地址的可达性分级。
///
/// - 点对点链路掩码（`/31`、`/32`）→ `2`（广播域里没有第二台设备）。
/// - 适配器名命中虚拟网卡关键字 → `2`。
/// - 其余私网地址 → `0`；公网地址 → `1`（能连但通常不是用户期望的局域网地址）。
fn reachability_tier(prefix_len: Option<u8>, virtual_adapter: bool, private: bool) -> u8 {
    if matches!(prefix_len, Some(31) | Some(32)) {
        return 2;
    }
    if virtual_adapter {
        return 2;
    }
    if private {
        0
    } else {
        1
    }
}

/// 生成访问 URL。绑定到具体地址时只返回该地址；绑定 `0.0.0.0`/`::` 时枚举网卡。
pub fn local_urls(host: &str, port: u16) -> Vec<String> {
    local_addresses(host, port)
        .into_iter()
        .map(|address| address.url)
        .collect()
}

/// 枚举候选访问地址，按「可达性优先」排序，并附带适配器与掩码等元信息。
///
/// 绑定到具体地址（非 `0.0.0.0`/`::`）时只返回该地址，元信息为占位值。
/// 无任何可用地址时回退 `127.0.0.1`（仅本机可见，tier 记为 `2`）。
pub fn local_addresses(host: &str, port: u16) -> Vec<LocalAddress> {
    if !host.is_empty() && host != "0.0.0.0" && host != "::" {
        return vec![LocalAddress {
            url: format!("http://{host}:{port}"),
            interface: "指定地址".to_string(),
            ip: host.to_string(),
            prefix_len: None,
            virtual_link: false,
            tier: 0,
        }];
    }

    let mut candidates: Vec<LocalAddress> = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            if interface.is_loopback() {
                continue;
            }
            let if_addrs::IfAddr::V4(ifv4) = interface.addr else {
                continue;
            };
            let ip = ifv4.ip;
            if ip.is_unspecified() || ip.is_loopback() {
                continue;
            }

            let name = interface.name;
            let prefix_len = Some(prefix_len_of(ifv4.netmask));
            let virtual_link = is_virtual_adapter(&name);
            candidates.push(LocalAddress {
                url: format!("http://{ip}:{port}"),
                interface: name,
                ip: ip.to_string(),
                prefix_len,
                virtual_link,
                tier: reachability_tier(prefix_len, virtual_link, ip.is_private()),
            });
        }
    }

    // 可达性优先 → 掩码更宽（网段更大）优先 → IP 数值升序。
    // 注意最后一步必须用 u32 数值，不能用 to_string() 的字典序。
    candidates.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then_with(|| b.prefix_len.unwrap_or(0).cmp(&a.prefix_len.unwrap_or(0)))
            .then_with(|| ip_sort_key(&a.ip).cmp(&ip_sort_key(&b.ip)))
    });

    // 借排序结果去重，保持既有顺序（同一 IP 只保留首个，即可达性最高的那条）。
    let mut seen: std::collections::HashSet<IpAddr> = std::collections::HashSet::new();
    candidates.retain(|address| match address.ip.parse::<IpAddr>() {
        Ok(ip) => seen.insert(ip),
        Err(_) => true,
    });

    if candidates.is_empty() {
        candidates.push(LocalAddress {
            url: format!("http://127.0.0.1:{port}"),
            interface: "本机回环".to_string(),
            ip: "127.0.0.1".to_string(),
            prefix_len: Some(8),
            virtual_link: false,
            tier: 2,
        });
    }
    candidates
}

/// 排序键：IPv4 走 `u32` 数值；IPv6 走原值兜底（当前只枚举 IPv4）。
fn ip_sort_key(ip: &str) -> u64 {
    match ip.parse::<std::net::Ipv4Addr>() {
        Ok(ipv4) => u64::from(u32::from(ipv4)),
        Err(_) => 0,
    }
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
    fn specific_host_is_always_recommended() {
        let addresses = local_addresses("192.168.1.5", 8080);
        assert_eq!(addresses.len(), 1);
        assert!(addresses[0].recommended());
    }

    #[test]
    fn wildcard_host_never_empty() {
        let urls = local_urls("0.0.0.0", 0);
        assert!(!urls.is_empty());
        assert!(urls.iter().all(|url| url.starts_with("http://")));
    }

    #[test]
    fn point_to_point_mask_is_lowest_tier() {
        // /32 点对点链路：即使落在私网段，也不该被当成主地址
        assert_eq!(reachability_tier(Some(32), false, true), 2);
        assert_eq!(reachability_tier(Some(31), false, true), 2);
        // /24 普通私网
        assert_eq!(reachability_tier(Some(24), false, true), 0);
        // 公网地址次之
        assert_eq!(reachability_tier(Some(24), false, false), 1);
        // 虚拟网卡即使掩码正常也降级
        assert_eq!(reachability_tier(Some(24), true, true), 2);
    }

    #[test]
    fn virtual_adapters_are_detected_by_name() {
        assert!(is_virtual_adapter("NodeBabyLink"));
        assert!(is_virtual_adapter("Tailscale"));
        assert!(is_virtual_adapter("vEthernet (WSL)"));
        assert!(is_virtual_adapter("VMware Network Adapter VMnet8"));
        assert!(is_virtual_adapter("vboxnet0"));
        assert!(!is_virtual_adapter("以太网"));
        assert!(!is_virtual_adapter("WLAN"));
    }

    #[test]
    fn prefix_len_counts_set_bits() {
        assert_eq!(prefix_len_of(std::net::Ipv4Addr::new(255, 255, 255, 0)), 24);
        assert_eq!(prefix_len_of(std::net::Ipv4Addr::new(255, 255, 252, 0)), 22);
        assert_eq!(
            prefix_len_of(std::net::Ipv4Addr::new(255, 255, 255, 255)),
            32
        );
    }

    #[test]
    fn ip_sort_key_uses_numeric_order() {
        // 字典序下 "10.222.222.1" < "10.80.40.166"（'2' < '8'），数值序必须反过来
        assert!(ip_sort_key("10.222.222.1") > ip_sort_key("10.80.40.166"));
        assert!(ip_sort_key("10.0.0.2") > ip_sort_key("10.0.0.1"));
    }

    #[test]
    fn wider_netmask_wins_within_same_tier() {
        // 同一 tier 内，/22 的以太网应排在 /32 之前——这里直接验证排序比较链
        let mut list = [
            ("10.222.222.1", Some(32), true, 2u8),
            ("10.80.40.166", Some(22), false, 0u8),
            ("100.87.166.93", Some(32), true, 2u8),
        ];
        list.sort_by(|a, b| {
            a.3.cmp(&b.3)
                .then_with(|| b.1.unwrap_or(0).cmp(&a.1.unwrap_or(0)))
                .then_with(|| ip_sort_key(a.0).cmp(&ip_sort_key(b.0)))
        });
        assert_eq!(list[0].0, "10.80.40.166");
    }
}
