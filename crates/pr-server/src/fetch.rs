//! Загрузка страницы по ссылке на сервере — с защитой от SSRF: сервер не должен
//! стать проходом во внутреннюю сеть. Адрес проверяется после разрешения DNS и
//! закрепляется за запросом (без повторного разрешения), каждое перенаправление
//! проверяется заново.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use reqwest::{Url, redirect};

const MAX_PAGE: usize = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const TIMEOUT: Duration = Duration::from_secs(15);
const USER_AGENT: &str = concat!(
    "PlainRussian/",
    env!("CARGO_PKG_VERSION"),
    " (+https://plainrussian.ru)"
);

pub struct Page {
    /// MIME-тип без параметров, в нижнем регистре.
    pub content_type: String,
    pub text: String,
}

/// Разрешать ли адреса внутренней сети. Только для тестов.
#[derive(Clone, Copy, Default)]
pub struct Policy {
    pub allow_private: bool,
}

pub async fn fetch(raw: &str, policy: Policy) -> Result<Page, String> {
    let mut url = Url::parse(raw.trim()).map_err(|_| format!("не похоже на адрес: {raw}"))?;
    for _ in 0..=MAX_REDIRECTS {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("поддерживаются только адреса http и https".into());
        }
        let host = url.host_str().ok_or("в адресе нет сервера")?.to_string();
        let port = url.port_or_known_default().ok_or("в адресе нет порта")?;
        let addr = resolve(&host, port, policy).await?;
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .redirect(redirect::Policy::none())
            .resolve(&host, addr)
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| format!("{host} не отвечает: {e}"))?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("перенаправление без адреса")?;
            url = url.join(location).map_err(|_| "неверное перенаправление")?;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!("сервер ответил: {}", response.status()));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = read_limited(response).await?;
        return Ok(Page {
            content_type: mime(&content_type),
            text: decode(&bytes, &content_type),
        });
    }
    Err("слишком много перенаправлений".into())
}

async fn resolve(host: &str, port: u16, policy: Policy) -> Result<SocketAddr, String> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| format!("сервер {host} не найден"))?
        .collect();
    let first = *addrs.first().ok_or(format!("сервер {host} не найден"))?;
    // Если хоть один адрес внутренний, отказываем: иначе DNS мог бы подсунуть его позже.
    if !policy.allow_private && addrs.iter().any(|a| !is_public(a.ip())) {
        return Err("адреса внутренней сети не загружаются".into());
    }
    Ok(first)
}

/// Адрес из публичного интернета: не локальный, не частный, не служебный.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public_v4(v4),
            None => is_public_v6(v6),
        },
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // 100.64.0.0/10, CGNAT
        || (a == 198 && (b == 18 || b == 19)) // 198.18.0.0/15, тесты сетей
        || a >= 240) // 240.0.0.0/4, зарезервировано
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00 // fc00::/7, уникальные локальные
        || (first & 0xffc0) == 0xfe80 // fe80::/10, локальные для канала
        || (first == 0x2001 && ip.segments()[1] == 0x0db8)) // 2001:db8::/32, документация
}

async fn read_limited(mut response: reqwest::Response) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_PAGE {
            return Err(format!("страница больше {} МБ", MAX_PAGE / 1024 / 1024));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn mime(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// Кодировка: из заголовка, из `<meta>`, по BOM; иначе UTF-8, а не-UTF-8 —
/// windows-1251. Та же логика, что в apps/desktop/src-tauri/src/net.rs.
pub fn decode(bytes: &[u8], content_type: &str) -> String {
    let declared = charset_param(content_type).or_else(|| meta_charset(bytes));
    if let Some(encoding) =
        declared.and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
    {
        return encoding.decode_with_bom_removal(bytes).0.into_owned();
    }
    if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding.decode_with_bom_removal(bytes).0.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::WINDOWS_1251.decode(bytes).0.into_owned(),
    }
}

fn charset_param(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|param| {
        let (name, value) = param.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches('"').to_string())
    })
}

fn meta_charset(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_ascii_lowercase();
    let start = head.find("charset=")? + "charset=".len();
    let value: String = head[start..]
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_and_special_addresses_are_not_public() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["93.184.216.34", "8.8.8.8", "2a00:1450:4010::8a"] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
    }

    #[tokio::test]
    async fn refuses_local_addresses_and_other_schemes() {
        let err = fetch("http://127.0.0.1:9/", Policy::default())
            .await
            .err()
            .unwrap();
        assert!(err.contains("внутренней сети"), "{err}");
        let err = fetch("http://localhost/", Policy::default())
            .await
            .err()
            .unwrap();
        assert!(err.contains("внутренней сети"), "{err}");
        let err = fetch("file:///etc/passwd", Policy::default())
            .await
            .err()
            .unwrap();
        assert!(err.contains("http"), "{err}");
    }
}
