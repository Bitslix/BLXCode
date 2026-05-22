# Theme exceptions

These surfaces **do not** follow the app theme selector. This is intentional.

| Surface | Reason |
|---------|--------|
| **Embedded browser iframe content** (Linux) | Foreign document — only app chrome (toolbar, frame) uses theme tokens. |
| **Native child webview** (Windows/macOS) | Rendered outside SPA CSS. |
| **`--memory-category-color`** | User-defined category swatch (data), not an app theme choice. |
| **`public/flags/*.svg`** | National flag colors must stay accurate. |
| **Third-party CDN CSS** (`xterm.min.css`) | Overridden via app CSS + xterm JS `theme` object synced to tokens. |

Everything else in the workbench should use tokens from `themes/tokens.css`.
