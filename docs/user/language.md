# UI Language

BLXCode ships with a **fully internationalized desktop UI**: menus, settings, workspace flows, agent panel, and the first-run EULA are available in multiple languages. Translations are compiled into the app, so missing strings are caught at build time instead of failing silently at runtime.

## Supported Languages

BLXCode currently supports **14 UI locales**:

| Language | BCP-47 tag |
|---|---|
| Deutsch | `de-DE` |
| English | `en-US` |
| Español | `es-ES` |
| Français | `fr-FR` |
| Magyar | `hu-HU` |
| Italiano | `it-IT` |
| 日本語 | `ja-JP` |
| 한국어 | `ko-KR` |
| Polski | `pl-PL` |
| Português (Brasil) | `pt-BR` |
| Русский | `ru-RU` |
| 简体中文 | `zh-CN` |
| 繁體中文 | `zh-TW` |

Related BCP-47 variants (for example `en-GB`, `es-MX`, or `zh-Hant`) map to the closest supported locale when detected automatically.

## Change The UI Language

1. Open the command palette with **Ctrl+Shift+P** (or **Cmd+Shift+P** on macOS).
2. Choose **BLXCode settings**.
3. In the settings sheet, select the **App** category on the left.
4. Use the **UI language** picker (flag + native language name). The change applies immediately and is saved for the next launch.

The picker supports keyboard navigation: **Arrow Up/Down** moves between options, **Enter** or **Space** selects, **Escape** closes the menu.

## First Launch And Persistence

On first launch, BLXCode picks a locale in this order:

1. A saved choice in local storage (`blxcode_locale_v1`), if present.
2. The system or browser language reported by the WebView.
3. **English (`en-US`)** as the default fallback.

The EULA gate on first launch uses the same locale, so legal text matches the UI language you see.

## Voice And Locale

Voice **speech-to-text** can follow the current UI locale. In **Voice** settings, set **STT language** to **Follow app** so transcription requests send a primary ISO-639-1 hint (for example `de` when the UI is `de-DE`). See [Voice: STT And TTS](voice.md) for other STT language modes.

## For Contributors

To add UI strings, locales, or EULA translations, see [Internationalization](../developer/i18n.md).
