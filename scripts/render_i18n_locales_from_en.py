#!/usr/bin/env python3
"""Regenerate src/i18n/locales/{de_de,es_es,fr_fr,...}.rs from en_us.rs using deep-translator.

Requires: pip install deep-translator (use a venv).

Run from repo root:
  # Full rewrite of every non-English locale (all strings translated from en)
  python scripts/render_i18n_locales_from_en.py

  # Only replace entries that are still identical to en_us.rs (keeps good rows)
  python scripts/render_i18n_locales_from_en.py --patch-english-matches

  # Same, but only some locale files (basename without .rs)
  python scripts/render_i18n_locales_from_en.py --patch-english-matches --locales es_es,ja_jp

  # Force re-translate specific I18nKey variants from English for all targets
  python scripts/render_i18n_locales_from_en.py --keys QkTitle,MemTabFiles
"""
from __future__ import annotations

import argparse
import re
import time
from pathlib import Path

from deep_translator import GoogleTranslator
from deep_translator.exceptions import TranslationNotFound

ROOT = Path(__file__).resolve().parents[1]
EN_US = ROOT / "src/i18n/locales/en_us.rs"
LOCALES_DIR = ROOT / "src/i18n/locales"

TARGETS: list[tuple[str, str, str]] = [
    ("de_de.rs", "de", "de-DE"),
    ("es_es.rs", "es", "es-ES"),
    ("fr_fr.rs", "fr", "fr-FR"),
    ("hu_hu.rs", "hu", "hu-HU"),
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


def parse_locale_rs(path: Path) -> list[tuple[str, str]]:
    """Extract (variant, string) in source order from a locales/*.rs msg() file."""
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


def emit_locale_rs(pairs: list[tuple[str, str]]) -> str:
    lines = [
        "use crate::i18n::I18nKey;",
        "",
        "#[must_use]",
        "pub fn msg(key: I18nKey) -> &'static str {",
        "    match key {",
    ]
    for k, tr in pairs:
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


def resolve_targets(locale_filter: str | None) -> list[tuple[str, str, str]]:
    if not locale_filter:
        return list(TARGETS)
    want = {x.strip() for x in locale_filter.split(",") if x.strip()}
    out: list[tuple[str, str, str]] = []
    for filename, code, label in TARGETS:
        base = filename.removesuffix(".rs")
        if base in want or filename in want:
            out.append((filename, code, label))
    if not out:
        raise SystemExit(f"no locales matched filter: {locale_filter!r}")
    return out


def run_full_regen(targets: list[tuple[str, str, str]]) -> None:
    pairs = parse_locale_rs(EN_US)
    if len(pairs) < 180:
        raise SystemExit(f"parse too few keys: {len(pairs)}")
    for filename, tgt_code, _label in targets:
        translator = GoogleTranslator(source="en", target=tgt_code)
        cache: dict[str, str] = {}
        merged: list[tuple[str, str]] = []
        for k, en in pairs:
            merged.append((k, translate_cached(translator, cache, en)))
        body = emit_locale_rs(merged)
        path = LOCALES_DIR / filename
        path.write_text(body, encoding="utf-8")
        print(f"wrote {path} ({len(cache)} unique strings translated)")
    print("done")


def run_patch_or_keys(
    *,
    patch_english: bool,
    keys: set[str] | None,
    targets: list[tuple[str, str, str]],
) -> None:
    en_pairs = parse_locale_rs(EN_US)
    if len(en_pairs) < 180:
        raise SystemExit(f"parse too few keys in en_us: {len(en_pairs)}")
    en_map = dict(en_pairs)
    if keys:
        missing = keys - set(en_map)
        if missing:
            raise SystemExit(f"unknown I18nKey names: {sorted(missing)}")

    for filename, tgt_code, _label in targets:
        path = LOCALES_DIR / filename
        loc_map = dict(parse_locale_rs(path))
        translator = GoogleTranslator(source="en", target=tgt_code)
        cache: dict[str, str] = {}
        merged: list[tuple[str, str]] = []
        for k, en in en_pairs:
            cur = loc_map.get(k, en)
            if keys and k in keys:
                cur = translate_cached(translator, cache, en)
            elif patch_english and cur == en:
                cur = translate_cached(translator, cache, en)
            merged.append((k, cur))
        path.write_text(emit_locale_rs(merged), encoding="utf-8")
        print(f"wrote {path} ({len(cache)} unique strings translated this run)")
    print("done")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--patch-english-matches",
        action="store_true",
        help="Keep each row unless it still matches English; then translate from en.",
    )
    ap.add_argument(
        "--keys",
        default="",
        help="Comma-separated I18nKey variant names (e.g. QkTitle,MemTabFiles) "
        "to always re-translate from English.",
    )
    ap.add_argument(
        "--locales",
        default="",
        help="Optional comma list of locale basenames (es_es, ja_jp, …). Default: all targets.",
    )
    args = ap.parse_args()
    targets = resolve_targets(args.locales.strip() or None)
    key_set = {x.strip() for x in args.keys.split(",") if x.strip()}

    if args.patch_english_matches or key_set:
        run_patch_or_keys(
            patch_english=args.patch_english_matches,
            keys=key_set or None,
            targets=targets,
        )
        return

    run_full_regen(targets)


if __name__ == "__main__":
    main()
