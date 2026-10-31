//! Внешние ссылки открываются в системном браузере. Иначе окно приложения ушло
//! бы на чужую страницу без кнопки «Назад», а введённый текст пропал бы.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_opener::OpenerExt;

/// Открывает ссылку в браузере. Страница вызывает её для всех внешних ссылок,
/// в том числе с `target="_blank"`, которые WebView сам не открывает.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let url = Url::parse(&url).map_err(|_| format!("не похоже на адрес: {url}"))?;
    if !is_web(&url) {
        return Err("открываются только ссылки http и https".into());
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Окно не уходит со страниц приложения: переход наружу открывается в браузере.
pub fn guard<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("links")
        .on_navigation(|webview, url| {
            if is_app(url) {
                return true;
            }
            if is_web(url) {
                let _ = webview.opener().open_url(url.as_str(), None::<&str>);
            }
            false
        })
        .build()
}

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Страницы самого приложения: tauri://localhost (macOS, Linux), http://tauri.localhost
/// (Windows) и сервер Vite при разработке.
fn is_app(url: &Url) -> bool {
    match url.scheme() {
        "tauri" => true,
        "http" | "https" => {
            let host = url.host_str().unwrap_or("");
            host == "tauri.localhost" || (cfg!(debug_assertions) && host == "localhost")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_app_pages_from_external_links() {
        for url in [
            "tauri://localhost/#settings",
            "http://tauri.localhost/index.html",
        ] {
            assert!(is_app(&Url::parse(url).unwrap()), "{url}");
        }
        for url in [
            "https://infoculture.ru/",
            "http://tauri.localhost.example.com/",
        ] {
            assert!(!is_app(&Url::parse(url).unwrap()), "{url}");
        }
        assert!(!is_web(&Url::parse("file:///etc/passwd").unwrap()));
    }
}
