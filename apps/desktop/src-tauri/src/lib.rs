//! Настольное приложение: оболочка Tauri над веб-интерфейсом из `apps/web`.
//!
//! Оценку считает то же ядро на WebAssembly, что и в браузере, поэтому
//! результаты совпадают. Rust-сторона даёт то, чего у страницы в браузере нет:
//! загрузку страницы по ссылке без ограничений CORS, HTTP-запросы к любому
//! LLM-провайдеру (GigaChat — с обменом ключа на токен), хранение ключей API
//! в системном хранилище и открытие внешних ссылок в браузере.

mod gigachat;
mod links;
mod net;
mod secrets;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(links::guard())
        .manage(gigachat::GigaChat::default())
        .invoke_handler(tauri::generate_handler![
            gigachat::gigachat_request,
            links::open_external,
            net::fetch_url,
            net::http_request,
            secrets::secret_get,
            secrets::secret_set,
            secrets::secret_delete,
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить приложение");
}
