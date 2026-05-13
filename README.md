# Tauri + Leptos

This template should help get you started developing with Tauri and Leptos.

## Better Auth backend (desktop client)

The Leptos UI talks to Better Auth over HTTP under `API_URL` + `API_PATH` (see [`src/config/app.config.rs`](src/config/app.config.rs)): session `auth/get-session`, email sign-in `auth/sign-in/email`, sign-out `auth/sign-out`, device flow `auth/device/code` and `auth/device/token`. Requests use `credentials: include` plus optional `Authorization: Bearer` after device login.

Configure the backend so that:

1. **`trustedOrigins`** includes the app origin (e.g. dev `http://localhost:1420` for Trunk/Tauri).
2. **CORS** allows that origin **without** `Access-Control-Allow-Origin: *` when using cookies; set **`Access-Control-Allow-Credentials: true`** where appropriate.
3. **Plugins**: enable **`deviceAuthorization`** (and migrations) so device codes work; register **`AUTH_DEVICE_CLIENT_ID`** (`blxcode-desktop` by default) if you use `validateClient`. If `/get-session` should accept the token from `/device/token`, enable the **[Bearer plugin](https://www.better-auth.com/docs/plugins/bearer)** as documented.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
