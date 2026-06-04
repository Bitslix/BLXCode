#![allow(
    clippy::clone_on_copy,
    clippy::collapsible_match,
    clippy::if_same_then_else,
    clippy::items_after_test_module,
    clippy::let_unit_value,
    clippy::manual_strip,
    clippy::match_like_matches_macro,
    clippy::map_identity,
    clippy::must_use_candidate,
    clippy::needless_borrow,
    clippy::only_used_in_recursion,
    clippy::question_mark,
    clippy::redundant_closure,
    clippy::redundant_locals,
    clippy::single_match,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unit_arg,
    clippy::unnecessary_map_or,
    clippy::unused_unit,
    reason = "Leptos view macros and Copy signals make these UI-style lints noisy at closure boundaries."
)]

mod agent_wire;
mod app;
mod app_log;
mod boot_loading;
mod config;
mod i18n;
mod memory_paths;
mod open_http;
mod quit;
mod service;
mod skills_rules_wire;
mod tauri_bridge;
mod theme;
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
