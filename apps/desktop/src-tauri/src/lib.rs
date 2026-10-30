//! Настольное приложение: оболочка Tauri над веб-интерфейсом из `apps/web`.
//!
//! Оценку считает то же ядро на WebAssembly, что и в браузере, поэтому
//! результаты совпадают. Rust-сторона даёт то, чего у страницы в браузере нет:
//! загрузку страницы по ссылке без ограничений CORS, HTTP-запросы к любому
//! LLM-провайдеру и хранение ключей API в системном хранилище.

mod net;
mod secrets;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            net::fetch_url,
            net::http_request,
            secrets::secret_get,
            secrets::secret_set,
            secrets::secret_delete,
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить приложение");
}
