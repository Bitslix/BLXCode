//! Title-bar brand cluster: BLXCode logo + name.
//! Always BLXCode — never the BridgeMind reference shell it is modeled on.
use leptos::prelude::*;

#[component]
pub fn TitleBarBrand() -> impl IntoView {
    view! {
        <div
            class="app-titlebar__brand"
            data-tauri-drag-region=""
            aria-label="BLXCode"
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
        </div>
    }
}
