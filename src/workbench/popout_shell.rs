use crate::tauri_bridge::PopoutPayload;
use leptos::prelude::*;

#[component]
pub fn PopoutShell(payload: PopoutPayload) -> impl IntoView {
    view! {
        <div class="workbench-popout-shell">
            <main class="workbench-popout-shell__body">
                <div class="workbench-popout-shell__placeholder">
                    <span class="workbench-popout-shell__eyebrow">"BLXCode Popout"</span>
                    <h1>{payload.fallback_title()}</h1>
                </div>
            </main>
        </div>
    }
}
