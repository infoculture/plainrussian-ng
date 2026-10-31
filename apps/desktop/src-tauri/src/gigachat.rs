//! GigaChat (Сбер): ключ авторизации обменивается на токен доступа на 30 минут,
//! токен хранится в памяти и обновляется сам. Из браузера GigaChat недоступен:
//! нет CORS, а сертификаты выпущены Минцифры, поэтому запросы делает Rust.
//!
//! Сертификат «Russian Trusted Root CA» берётся из системного хранилища: если его
//! там нет, пользователь получает ссылку на инструкцию Сбера по установке.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Url;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::net::{HttpResponse, MAX_RESPONSE, client, describe, read_limited};

const SCOPES: &[&str] = &["GIGACHAT_API_PERS", "GIGACHAT_API_B2B", "GIGACHAT_API_CORP"];
/// Пути API, которые нужны интерфейсу: оценка текста и список моделей.
const PATHS: &[&str] = &["/chat/completions", "/models"];
/// Токен обновляется заранее, чтобы не истечь посреди запроса.
const REFRESH_MARGIN_MS: u64 = 60_000;
const CERTIFICATES_URL: &str = "https://developers.sber.ru/docs/ru/gigachat/certificates";

/// Адреса API; в тестах подменяются локальным сервером.
#[derive(Clone)]
pub struct Endpoints {
    pub oauth: String,
    pub api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            oauth: "https://ngw.devices.sberbank.ru:9443/api/v2/oauth".into(),
            api: "https://api.giga.chat/v1".into(),
        }
    }
}

struct Token {
    /// Для какого ключа и типа доступа выдан: при смене ключа нужен новый.
    owner: (String, String),
    access_token: String,
    expires_at_ms: u64,
}

#[derive(Default)]
pub struct GigaChat {
    endpoints: Endpoints,
    token: Mutex<Option<Token>>,
}

impl GigaChat {
    #[cfg(test)]
    fn with_endpoints(endpoints: Endpoints) -> Self {
        Self {
            endpoints,
            token: Mutex::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GigaChatRequest {
    /// Ключ авторизации из личного кабинета (Base64 от Client ID и Client Secret).
    auth_key: String,
    scope: String,
    method: String,
    path: String,
    body: Option<String>,
}

#[tauri::command]
pub async fn gigachat_request(
    state: tauri::State<'_, GigaChat>,
    request: GigaChatRequest,
) -> Result<HttpResponse, String> {
    state.request(request).await
}

impl GigaChat {
    async fn request(&self, request: GigaChatRequest) -> Result<HttpResponse, String> {
        if !SCOPES.contains(&request.scope.as_str()) {
            return Err(format!("Неизвестный тип доступа: {}", request.scope));
        }
        if !PATHS.contains(&request.path.as_str()) {
            return Err(format!("Путь {} не поддерживается", request.path));
        }
        if !matches!(request.method.as_str(), "GET" | "POST") {
            return Err(format!("Метод {} не поддерживается", request.method));
        }
        let token = self.token(&request, false).await?;
        let response = self.call(&token, &request).await?;
        if response.status != 401 {
            return Ok(response);
        }
        // Токен могли отозвать раньше срока: получаем новый и повторяем один раз.
        let token = self.token(&request, true).await?;
        self.call(&token, &request).await
    }

    async fn token(&self, request: &GigaChatRequest, refresh: bool) -> Result<String, String> {
        let owner = (request.auth_key.clone(), request.scope.clone());
        let mut slot = self.token.lock().await;
        if let Some(token) = slot.as_ref()
            && !refresh
            && token.owner == owner
            && token.expires_at_ms > now_ms() + REFRESH_MARGIN_MS
        {
            return Ok(token.access_token.clone());
        }
        let url = Url::parse(&self.endpoints.oauth).map_err(|e| e.to_string())?;
        let response = client(Duration::from_secs(30), &url)?
            .post(url.clone())
            .header(
                "Authorization",
                format!("Basic {}", request.auth_key.trim()),
            )
            .header("RqUID", uuid::Uuid::new_v4().to_string())
            .header("Accept", "application/json")
            .form(&[("scope", request.scope.as_str())])
            .send()
            .await
            .map_err(|e| explain(&e, &url))?;
        let status = response.status();
        let bytes = read_limited(response, MAX_RESPONSE, &url).await?;
        if status.as_u16() == 401 || status.as_u16() == 400 {
            return Err(
                "Ключ авторизации GigaChat не подошёл. Проверьте ключ и тип доступа.".into(),
            );
        }
        if !status.is_success() {
            return Err(format!("GigaChat не выдал токен: {status}"));
        }
        #[derive(Deserialize)]
        struct OAuth {
            access_token: String,
            expires_at: u64,
        }
        let oauth: OAuth = serde_json::from_slice(&bytes)
            .map_err(|_| "GigaChat вернул неожиданный ответ при выдаче токена".to_string())?;
        let token = oauth.access_token.clone();
        *slot = Some(Token {
            owner,
            access_token: oauth.access_token,
            expires_at_ms: to_ms(oauth.expires_at),
        });
        Ok(token)
    }

    async fn call(&self, token: &str, request: &GigaChatRequest) -> Result<HttpResponse, String> {
        let url = Url::parse(&format!("{}{}", self.endpoints.api, request.path))
            .map_err(|e| e.to_string())?;
        let client = client(Duration::from_secs(300), &url)?;
        let mut builder = if request.method == "GET" {
            client.get(url.clone())
        } else {
            client.post(url.clone())
        };
        builder = builder
            .bearer_auth(token)
            .header("Accept", "application/json");
        if let Some(body) = &request.body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.clone());
        }
        let response = builder.send().await.map_err(|e| explain(&e, &url))?;
        let status = response.status().as_u16();
        let bytes = read_limited(response, MAX_RESPONSE, &url).await?;
        Ok(HttpResponse {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}

/// Ошибка проверки сертификата — самая частая причина: в системе нет сертификата Минцифры.
fn explain(e: &reqwest::Error, url: &Url) -> String {
    let mut source: Option<&dyn std::error::Error> = Some(e);
    while let Some(cause) = source {
        let text = cause.to_string().to_lowercase();
        if text.contains("certificate")
            || text.contains("unknownissuer")
            || text.contains("unknown issuer")
        {
            return format!(
                "Система не доверяет сертификату GigaChat. Установите сертификат Минцифры \
                 «Russian Trusted Root CA» по инструкции Сбера: {CERTIFICATES_URL}"
            );
        }
        source = cause.source();
    }
    describe(e, url)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// В документации `expires_at` — время Unix; встречаются и секунды, и миллисекунды.
fn to_ms(expires_at: u64) -> u64 {
    if expires_at > 100_000_000_000 {
        expires_at
    } else {
        expires_at * 1000
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex as StdMutex};

    use super::*;

    /// Локальный сервер: отвечает по очереди заданными ответами и записывает запросы.
    fn serve(responses: Vec<String>) -> (String, Arc<StdMutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let log = Arc::new(StdMutex::new(Vec::new()));
        let seen = log.clone();
        std::thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap();
                seen.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        (format!("http://{addr}"), log)
    }

    fn http(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn token_json(token: &str) -> String {
        format!(
            r#"{{"access_token":"{token}","expires_at":{}}}"#,
            now_ms() + 30 * 60_000
        )
    }

    fn chat_request() -> GigaChatRequest {
        GigaChatRequest {
            auth_key: "a2V5".into(),
            scope: "GIGACHAT_API_PERS".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            body: Some(r#"{"model":"GigaChat-2-Max"}"#.into()),
        }
    }

    #[tokio::test]
    async fn exchanges_key_once_and_reuses_token() {
        let (oauth, oauth_log) = serve(vec![http("200 OK", &token_json("t1"))]);
        let (api, api_log) = serve(vec![http("200 OK", "{}"), http("200 OK", "{}")]);
        let giga = GigaChat::with_endpoints(Endpoints {
            oauth: format!("{oauth}/oauth"),
            api,
        });
        assert_eq!(giga.request(chat_request()).await.unwrap().status, 200);
        assert_eq!(giga.request(chat_request()).await.unwrap().status, 200);

        let oauth_log = oauth_log.lock().unwrap();
        assert_eq!(oauth_log.len(), 1, "токен запрошен один раз");
        let token_request = oauth_log[0].to_lowercase();
        assert!(token_request.contains("authorization: basic a2v5"));
        assert!(token_request.contains("rquid: "));
        assert!(token_request.contains("scope=gigachat_api_pers"));
        let api_log = api_log.lock().unwrap();
        assert!(api_log[0].starts_with("POST /chat/completions"));
        assert!(
            api_log[1]
                .to_lowercase()
                .contains("authorization: bearer t1")
        );
    }

    #[tokio::test]
    async fn refreshes_token_after_401() {
        let (oauth, oauth_log) = serve(vec![
            http("200 OK", &token_json("old")),
            http("200 OK", &token_json("new")),
        ]);
        let (api, api_log) = serve(vec![http("401 Unauthorized", "{}"), http("200 OK", "{}")]);
        let giga = GigaChat::with_endpoints(Endpoints {
            oauth: format!("{oauth}/oauth"),
            api,
        });
        assert_eq!(giga.request(chat_request()).await.unwrap().status, 200);
        assert_eq!(oauth_log.lock().unwrap().len(), 2);
        assert!(
            api_log.lock().unwrap()[1]
                .to_lowercase()
                .contains("bearer new")
        );
    }

    #[tokio::test]
    async fn bad_key_and_bad_requests_are_explained() {
        let (oauth, _) = serve(vec![http("401 Unauthorized", "{}")]);
        let giga = GigaChat::with_endpoints(Endpoints {
            oauth: format!("{oauth}/oauth"),
            api: "http://127.0.0.1:9".into(),
        });
        let err = giga.request(chat_request()).await.unwrap_err();
        assert!(err.contains("не подошёл"), "{err}");

        let mut wrong = chat_request();
        wrong.scope = "ANY".into();
        assert!(
            giga.request(wrong)
                .await
                .unwrap_err()
                .contains("тип доступа")
        );
        let mut wrong = chat_request();
        wrong.path = "/files".into();
        assert!(giga.request(wrong).await.unwrap_err().contains("Путь"));
    }

    /// Сервер с сертификатом от неизвестного центра — как GigaChat без сертификата Минцифры.
    #[tokio::test]
    #[ignore = "нужен интернет"]
    async fn untrusted_certificate_is_explained() {
        let url = Url::parse("https://untrusted-root.badssl.com/").unwrap();
        let err = client(Duration::from_secs(15), &url)
            .unwrap()
            .get(url.clone())
            .send()
            .await
            .unwrap_err();
        let message = explain(&err, &url);
        assert!(message.contains(CERTIFICATES_URL), "{message}");
    }

    #[test]
    fn expires_at_in_seconds_or_milliseconds() {
        assert_eq!(to_ms(1_679_471_442), 1_679_471_442_000);
        assert_eq!(to_ms(1_706_026_848_841), 1_706_026_848_841);
    }
}
