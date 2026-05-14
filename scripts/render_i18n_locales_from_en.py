#!/usr/bin/env python3
"""Regenerate src/i18n/locales/{es_es,fr_fr,...}.rs from en_us.rs using deep-translator.

Requires: pip install deep-translator (use a venv).

Run from repo root:
  python scripts/render_i18n_locales_from_en.py
"""
from __future__ import annotations

import re
import time
from pathlib import Path

from deep_translator import GoogleTranslator
from deep_translator.exceptions import TranslationNotFound

ROOT = Path(__file__).resolve().parents[1]
EN_US = ROOT / "src/i18n/locales/en_us.rs"

TARGETS: list[tuple[str, str, str]] = [
    ("es_es.rs", "es", "es-ES"),
    ("fr_fr.rs", "fr", "fr-FR"),
    ("pt_br.rs", "pt", "pt-BR"),
    ("it_it.rs", "it", "it-IT"),
    ("pl_pl.rs", "pl", "pl-PL"),
    ("ru_ru.rs", "ru", "ru-RU"),
    ("ja_jp.rs", "ja", "ja-JP"),
    ("ko_kr.rs", "ko", "ko-KR"),
    ("zh_cn.rs", "zh-CN", "zh-CN"),
    ("zh_tw.rs", "zh-TW", "zh-TW"),
]


def rust_escape(s: str) -> str:
    out: list[str] = []
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        else:
            out.append(ch)
    return "".join(out)


def parse_en_us(path: Path) -> list[tuple[str, str]]:
    """Extract (variant, string) in source order from en_us.rs."""
    text = path.read_text(encoding="utf-8")
    out: list[tuple[str, str]] = []
    for m in re.finditer(
        r"I18nKey::(\w+)\s*=>\s*(?:\{\s*\"((?:[^\"\\]|\\.)*)\"\s*\}|\"((?:[^\"\\]|\\.)*)\")\s*,?",
        text,
        re.S,
    ):
        key = m.group(1)
        braced, inline = m.group(2), m.group(3)
        s = braced if braced is not None else (inline or "")
        out.append((key, s.replace("\\n", "\n")))
    return out


def translate_cached(
    translator: GoogleTranslator, cache: dict[str, str], text: str
) -> str:
    if text in cache:
        return cache[text]
    for attempt in range(4):
        try:
            t = translator.translate(text)
            cache[text] = t
            time.sleep(0.1)
            return t
        except TranslationNotFound:
            cache[text] = text
            return text
        except Exception:  # noqa: BLE001 — network flakiness
            if attempt == 3:
                cache[text] = text
                return text
            time.sleep(0.6 * (attempt + 1))
    cache[text] = text
    return text


def render_locale(
    filename: str,
    tgt_code: str,
    pairs: list[tuple[str, str]],
    cache: dict[str, str],
    translator: GoogleTranslator,
) -> str:
    lines = [
        "use crate::i18n::I18nKey;",
        "",
        "#[must_use]",
        "pub fn msg(key: I18nKey) -> &'static str {",
        "    match key {",
    ]
    for k, en in pairs:
        tr = translate_cached(translator, cache, en)
        esc = rust_escape(tr)
        if "\n" in tr or len(tr) > 100:
            lines.append(f"        I18nKey::{k} => {{")
            lines.append(f'            "{esc}"')
            lines.append("        }")
        else:
            lines.append(f'        I18nKey::{k} => "{esc}",')
    lines.append("    }")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    pairs = parse_en_us(EN_US)
    if len(pairs) < 180:
        raise SystemExit(f"parse too few keys: {len(pairs)}")
    for filename, tgt_code, _label in TARGETS:
        translator = GoogleTranslator(source="en", target=tgt_code)
        cache: dict[str, str] = {}
        body = render_locale(filename, tgt_code, pairs, cache, translator)
        path = ROOT / "src/i18n/locales" / filename
        path.write_text(body, encoding="utf-8")
        print(f"wrote {path} ({len(cache)} unique strings translated)")
    print("done")


if __name__ == "__main__":
    main()
