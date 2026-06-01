//! Title-bar brand cluster: BLXCode logo + name + version badge.
//! Always BLXCode — never the BridgeMind reference shell it is modeled on.
use leptos::prelude::*;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[component]
pub fn TitleBarBrand() -> impl IntoView {
    view! {
        <div
            class="app-titlebar__brand"
            data-tauri-drag-region=""
            aria-label=format!("BLXCode v{APP_VERSION}")
        >
            <img
                class="app-titlebar__brand-logo"
                src="/public/blxcode.png"
                alt=""
                width="20"
                height="20"
                decoding="async"
                draggable="false"
            />
            <span class="app-titlebar__brand-name">"BLXCode"</span>
            <span class="app-titlebar__brand-version">{format!("v{APP_VERSION}")}</span>
        </div>
    }
}
