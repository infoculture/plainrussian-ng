//! HTTP API в формате старого plainrussian.ru: `/api/1.0/ru/measure/`.
//!
//! Принимает то же, что старый сервис: GET с `url=` или `text=`, POST формой
//! (`text=...`) или JSON. Отвечает теми же полями `metrics`, `indexes` и `status`,
//! посчитанными старым алгоритмом, чтобы прежние интеграции получали прежние числа,
//! и добавляет `readability` — оценку текущим алгоритмом, а по `clarity=1` — проверки
//! ясного языка. Тексты не сохраняются и не пишутся в журнал.

pub mod fetch;
pub mod html;

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use pr_core::clarity::{self, Dictionary};
use pr_core::{legacy, readability};
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;

/// Код ошибки старого API: неверные данные.
const ERROR_INVALID_DATA: u32 = 101;
/// Текст длиннее этого не оценивается.
const MAX_TEXT_CHARS: usize = 1_000_000;
const MAX_BODY: usize = 4 * MAX_TEXT_CHARS;

pub struct AppState {
    pub dictionary: Dictionary,
    pub fetch_policy: fetch::Policy,
}

pub fn app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);
    Router::new()
        .route("/api/1.0/ru/measure/", get(measure_get).post(measure_post))
        .route("/api/1.0/ru/measure", get(measure_get).post(measure_post))
        .route("/health", get(|| async { "ok" }))
        .layer(RequestBodyLimitLayer::new(MAX_BODY))
        .layer(cors)
        .with_state(Arc::new(state))
}

#[derive(Deserialize, Default)]
struct Params {
    text: Option<String>,
    url: Option<String>,
    debug: Option<String>,
    clarity: Option<String>,
}

fn flag(value: &Option<String>) -> bool {
    matches!(value.as_deref(), Some("1" | "true" | "yes"))
}

async fn measure_get(State(state): State<Arc<AppState>>, Query(params): Query<Params>) -> Response {
    respond(state, params).await
}

async fn measure_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    let params = if json {
        serde_json::from_slice::<Params>(&body).map_err(|e| e.to_string())
    } else {
        serde_urlencoded::from_bytes::<Params>(&body).map_err(|e| e.to_string())
    };
    match params {
        Ok(params) => respond(state, params).await,
        Err(e) => error(
            StatusCode::BAD_REQUEST,
            &format!("не разобрать запрос: {e}"),
        ),
    }
}

async fn respond(state: Arc<AppState>, params: Params) -> Response {
    let text = match (&params.text, &params.url) {
        (Some(text), _) if !text.trim().is_empty() => text.clone(),
        (_, Some(url)) if !url.trim().is_empty() => match load(url, state.fetch_policy).await {
            Ok(text) => text,
            Err(e) => return error(StatusCode::BAD_REQUEST, &e),
        },
        _ => return error(StatusCode::BAD_REQUEST, "нужен параметр text или url"),
    };
    if text.chars().count() > MAX_TEXT_CHARS {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "текст длиннее миллиона знаков",
        );
    }
    let with_clarity = flag(&params.clarity);
    let debug = flag(&params.debug);
    // Оценка — работа для процессора, а не для асинхронного потока.
    let result = tokio::task::spawn_blocking(move || {
        let old = legacy::measure(&text);
        let mut body = json!({
            "status": 0,
            "metrics": old.metrics,
            "indexes": old.indexes,
            "readability": readability::measure(&text),
        });
        if with_clarity {
            body["clarity"] = json!(clarity::check(&state.dictionary, &text));
        }
        if debug {
            body["debug"] = json!({ "text": text });
        }
        body
    })
    .await;
    match result {
        Ok(body) => Json(body).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "оценка не удалась"),
    }
}

async fn load(url: &str, policy: fetch::Policy) -> Result<String, String> {
    let page = fetch::fetch(url, policy).await?;
    match page.content_type.as_str() {
        "text/html" | "application/xhtml+xml" | "" => Ok(html::to_text(&page.text)),
        "text/plain" | "text/markdown" => Ok(page.text),
        other => Err(format!("по ссылке не текст и не веб-страница, а {other}")),
    }
}

fn error(status: StatusCode, message: &str) -> Response {
    let body: Value = json!({ "status": ERROR_INVALID_DATA, "error": message });
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    const TEXT: &str = "В целях обеспечения реализации мероприятий осуществляется проведение мониторинга. Мама мыла раму.";

    fn state(allow_private: bool) -> AppState {
        AppState {
            dictionary: Dictionary::bundled().unwrap(),
            fetch_policy: fetch::Policy { allow_private },
        }
    }

    async fn call(request: Request<Body>, allow_private: bool) -> (StatusCode, HeaderMap, Value) {
        let response = app(state(allow_private)).oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, headers, serde_json::from_slice(&bytes).unwrap())
    }

    fn form(body: String) -> Request<Body> {
        Request::post("/api/1.0/ru/measure/")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, "https://example.org")
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn post_form_like_the_old_api() {
        let body = serde_urlencoded::to_string([("text", TEXT)]).unwrap();
        let (status, headers, json) = call(form(body), false).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], 0);
        let old = legacy::measure(TEXT);
        assert_eq!(json["indexes"]["index_SMOG"], json!(old.indexes.index_smog));
        assert_eq!(json["metrics"]["n_words"], json!(old.metrics.n_words));
        assert_eq!(
            json["readability"]["counts"]["sentences"],
            json!(readability::measure(TEXT).counts.sentences)
        );
        assert!(json.get("clarity").is_none() && json.get("debug").is_none());
        assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    }

    #[tokio::test]
    async fn post_json_with_clarity_and_debug() {
        let request = Request::post("/api/1.0/ru/measure")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "text": TEXT, "clarity": "1", "debug": "1" }).to_string(),
            ))
            .unwrap();
        let (status, _, json) = call(request, false).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["clarity"]["counts"]["genitive_chain"], 1);
        assert_eq!(json["debug"]["text"], TEXT);
    }

    #[tokio::test]
    async fn get_with_text_and_errors() {
        let query = serde_urlencoded::to_string([("text", "Кот спал. Пёс лаял.")]).unwrap();
        let request = Request::get(format!("/api/1.0/ru/measure/?{query}"))
            .body(Body::empty())
            .unwrap();
        let (status, _, json) = call(request, false).await;
        assert_eq!((status, json["status"].clone()), (StatusCode::OK, json!(0)));

        let request = Request::get("/api/1.0/ru/measure/")
            .body(Body::empty())
            .unwrap();
        let (status, _, json) = call(request, false).await;
        assert_eq!(
            (status, json["status"].clone()),
            (StatusCode::BAD_REQUEST, json!(101))
        );

        let request = Request::get("/api/1.0/ru/measure/?url=http://127.0.0.1:9/")
            .body(Body::empty())
            .unwrap();
        let (status, _, json) = call(request, false).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["error"].as_str().unwrap().contains("внутренней сети"));
    }

    /// Сервер на один ответ для проверки загрузки по ссылке.
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
    async fn get_url_extracts_the_page_text() {
        let (page, _, _) = encoding_rs::WINDOWS_1251
            .encode("<html><body><nav>Меню</nav><article><p>Мама мыла раму. Папа читал газету.</p></article></body></html>");
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=windows-1251\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            page.len()
        )
        .into_bytes();
        response.extend_from_slice(&page);
        let url = serve_once(response);
        let query = serde_urlencoded::to_string([("url", url.as_str()), ("debug", "1")]).unwrap();
        let request = Request::get(format!("/api/1.0/ru/measure/?{query}"))
            .body(Body::empty())
            .unwrap();
        let (status, _, json) = call(request, true).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(json["debug"]["text"], "Мама мыла раму. Папа читал газету.");
        assert_eq!(json["readability"]["counts"]["words"], 6);
    }
}
