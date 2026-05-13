mod agent_wire;
mod app;
mod auth;
mod config;
mod i18n;
mod quit;
mod service;
mod tauri_bridge;
mod workbench;

use app::*;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! {
            <App/>
        }
    })
}
