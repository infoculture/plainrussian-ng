//! `plainrussian-server` — HTTP API в формате старого plainrussian.ru.
//!
//!     plainrussian-server                         слушает 127.0.0.1:9888, как старый сервис
//!     plainrussian-server --listen 0.0.0.0:8080
//!
//!     curl -d 'text=Мама мыла раму.' http://127.0.0.1:9888/api/1.0/ru/measure/

use std::net::SocketAddr;

use clap::Parser;
use pr_core::clarity::Dictionary;
use pr_server::{AppState, app, fetch};

#[derive(Parser)]
#[command(
    name = "plainrussian-server",
    version,
    about = "HTTP API в формате старого plainrussian.ru: /api/1.0/ru/measure/"
)]
struct Args {
    /// Адрес и порт.
    #[arg(long, default_value = "127.0.0.1:9888")]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let dictionary = Dictionary::bundled().expect("встроенный словарь rumorph не читается");
    let state = AppState {
        dictionary,
        fetch_policy: fetch::Policy::default(),
    };
    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .unwrap_or_else(|e| panic!("не удалось занять {}: {e}", args.listen));
    eprintln!(
        "plainrussian-server: http://{}/api/1.0/ru/measure/",
        args.listen
    );
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("сервер остановился с ошибкой");
}
