//! Сеть без ограничений браузера: страница по ссылке и запросы к LLM-провайдерам.
//!
//! В браузере обе вещи упираются в CORS, а запросы к LLM ещё и в CSP страницы.
//! Здесь запросы делает Rust, а интерфейс получает готовый текст.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use reqwest::Url;
use serde::{Deserialize, Serialize};

/// Больше этого страница не загружается: обычная статья занимает сотни килобайт.
const MAX_PAGE: usize = 20 * 1024 * 1024;
pub(crate) const MAX_RESPONSE: usize = 10 * 1024 * 1024;
const USER_AGENT: &str = concat!(
    "PlainRussian/",
    env!("CARGO_PKG_VERSION"),
    " (+https://plainrussian.ru)"
);

#[derive(Debug, Serialize)]
pub struct Page {
    /// Адрес после перенаправлений.
    url: String,
    /// MIME-тип без параметров, в нижнем регистре.
    content_type: String,
    text: String,
}

#[tauri::command]
pub async fn fetch_url(url: String) -> Result<Page, String> {
    let url = http_url(&url)?;
    let response = client(Duration::from_secs(30), &url)?
        .get(url.clone())
        .send()
        .await
        .map_err(|e| describe(&e, &url))?;
    if !response.status().is_success() {
        return Err(format!("Сервер ответил: {}", response.status()));
    }
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = read_limited(response, MAX_PAGE, &url).await?;
    Ok(Page {
        url: final_url,
        content_type: mime(&content_type),
        text: decode(&bytes, &content_type),
    })
}

#[derive(Debug, Deserialize)]
pub struct HttpRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HttpResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

/// Запрос к API LLM-провайдера. Только GET и POST, только http и https.
#[tauri::command]
pub async fn http_request(request: HttpRequest) -> Result<HttpResponse, String> {
    let url = http_url(&request.url)?;
    // Локальные модели на слабом компьютере отвечают минутами.
    let client = client(Duration::from_secs(600), &url)?;
    let mut builder = match request.method.as_str() {
        "GET" => client.get(url.clone()),
        "POST" => client.post(url.clone()),
        other => return Err(format!("Метод {other} не поддерживается")),
    };
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }
    let response = builder.send().await.map_err(|e| describe(&e, &url))?;
    let status = response.status().as_u16();
    let bytes = read_limited(response, MAX_RESPONSE, &url).await?;
    Ok(HttpResponse {
        status,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

fn http_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|_| format!("Не похоже на адрес: {raw}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(format!("Поддерживаются только http и https, а не {other}")),
    }
}

pub(crate) fn client(timeout: Duration, url: &Url) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout);
    // Прокси из HTTP_PROXY или настроек системы не видит серверов на этом компьютере:
    // запрос к LM Studio на localhost уходит в прокси и пропадает там.
    if is_loopback(url) {
        builder = builder.no_proxy();
    }
    builder.build().map_err(|e| e.to_string())
}

/// localhost, *.localhost, 127.0.0.0/8 и ::1.
fn is_loopback(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    match host.parse::<IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => {
            let host = host.trim_end_matches('.');
            host == "localhost" || host.ends_with(".localhost")
        }
    }
}

pub(crate) fn describe(e: &reqwest::Error, url: &Url) -> String {
    let host = url.host_str().unwrap_or("сервер");
    if e.is_timeout() {
        format!("{host} не ответил вовремя")
    } else if e.is_connect() {
        format!("Не удалось соединиться с {host}")
    } else {
        // Сама ошибка reqwest говорит только «error sending request», причина глубже.
        let mut message = format!("Ошибка запроса к {host}: {e}");
        let mut source = std::error::Error::source(e);
        while let Some(cause) = source {
            message.push_str(&format!(": {cause}"));
            source = cause.source();
        }
        message
    }
}

pub(crate) async fn read_limited(
    mut response: reqwest::Response,
    limit: usize,
    url: &Url,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| describe(&e, url))? {
        if bytes.len() + chunk.len() > limit {
            return Err(format!("Ответ больше {} МБ", limit / 1024 / 1024));
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

/// Кодировка: из заголовка, из `<meta>` в начале страницы, по BOM; иначе UTF-8.
/// Старые русские сайты часто в windows-1251 и пишут это только в `<meta>`.
/// Если ни то ни другое не указано, а байты не UTF-8, это почти наверняка windows-1251.
fn decode(bytes: &[u8], content_type: &str) -> String {
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

/// `<meta charset="...">` или `<meta http-equiv="Content-Type" content="...; charset=...">`
/// в первых 4 КБ страницы.
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
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn decodes_by_header_meta_bom_and_guess() {
        let (cp1251, _, _) = encoding_rs::WINDOWS_1251.encode("Привет, мир");
        assert_eq!(
            decode(&cp1251, "text/plain; charset=windows-1251"),
            "Привет, мир"
        );

        let mut page = b"<html><head><meta charset=\"windows-1251\"></head><body>".to_vec();
        page.extend_from_slice(&cp1251);
        assert!(decode(&page, "text/html").contains("Привет, мир"));

        let mut page =
            b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=cp1251\">".to_vec();
        page.extend_from_slice(&cp1251);
        assert!(decode(&page, "text/html").contains("Привет, мир"));

        assert_eq!(decode("\u{feff}Текст".as_bytes(), ""), "Текст");
        assert_eq!(decode("Текст".as_bytes(), "text/html"), "Текст");
        assert_eq!(decode(&cp1251, ""), "Привет, мир");
    }

    #[test]
    fn accepts_only_http_urls() {
        assert!(http_url("https://plainrussian.ru/").is_ok());
        assert!(http_url(" http://localhost:11434/v1 ").is_ok());
        assert!(http_url("file:///etc/passwd").is_err());
        assert!(http_url("javascript:alert(1)").is_err());
        assert!(http_url("просто текст").is_err());
    }

    #[test]
    fn recognizes_loopback_hosts() {
        for url in [
            "http://localhost:1234/v1",
            "http://LocalHost./",
            "http://lm.localhost/",
            "http://127.0.0.1:11434/",
            "http://127.1.2.3/",
            "http://[::1]:1234/",
        ] {
            assert!(is_loopback(&Url::parse(url).unwrap()), "{url}");
        }
        for url in [
            "https://api.openai.com/v1",
            "http://192.168.1.10:1234/",
            "http://localhost.example.com/",
            "http://[::2]/",
        ] {
            assert!(!is_loopback(&Url::parse(url).unwrap()), "{url}");
        }
    }

    /// Сервер на один ответ: отдаёт `response` первому подключению.
    fn serve_once(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            stream.write_all(&response).unwrap();
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn fetches_a_windows_1251_page() {
        let (body, _, _) = encoding_rs::WINDOWS_1251.encode("<p>Мама мыла раму.</p>");
        let mut response =
            format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=windows-1251\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                .into_bytes();
        response.extend_from_slice(&body);
        let page = fetch_url(serve_once(response)).await.unwrap();
        assert_eq!(page.content_type, "text/html");
        assert_eq!(page.text, "<p>Мама мыла раму.</p>");
    }

    #[tokio::test]
    async fn reports_http_errors() {
        let response =
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
        let error = fetch_url(serve_once(response)).await.unwrap_err();
        assert!(error.contains("404"), "{error}");
    }

    #[tokio::test]
    async fn proxies_post_requests() {
        let response =
            b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 7\r\nConnection: close\r\n\r\nbad key"
                .to_vec();
        let result = http_request(HttpRequest {
            method: "POST".into(),
            url: serve_once(response),
            headers: HashMap::from([("Content-Type".into(), "application/json".into())]),
            body: Some("{}".into()),
        })
        .await
        .unwrap();
        assert_eq!(result.status, 401);
        assert_eq!(result.body, "bad key");
    }

    /// LM Studio и Ollama слушают localhost. Если в окружении задан HTTP_PROXY,
    /// запрос не должен уходить в прокси.
    #[tokio::test]
    async fn reaches_localhost_past_proxy() {
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec();
        let url = serve_once(response).replace("127.0.0.1", "localhost");
        let result = http_request(HttpRequest {
            method: "GET".into(),
            url,
            headers: HashMap::new(),
            body: None,
        })
        .await
        .unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body, "{}");
    }

    #[tokio::test]
    async fn rejects_other_methods() {
        let error = http_request(HttpRequest {
            method: "DELETE".into(),
            url: "http://127.0.0.1:9/".into(),
            headers: HashMap::new(),
            body: None,
        })
        .await
        .unwrap_err();
        assert!(error.contains("DELETE"));
    }
}
