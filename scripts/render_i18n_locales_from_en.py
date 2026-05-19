#!/usr/bin/env python3
"""Regenerate src/i18n/locales/{de_de,es_es,fr_fr,...}.rs from en_us.rs using deep-translator.

Requires: pip install deep-translator (use a venv).

Run from repo root:
  # Default: translate only I18nKey rows missing from each locale file (new keys in en_us.rs)
  python scripts/render_i18n_locales_from_en.py

  # Also re-translate rows that still match English verbatim (use sparingly)
  python scripts/render_i18n_locales_from_en.py --patch-english-matches

  # Only specific keys (comma-separated variant names)
  python scripts/render_i18n_locales_from_en.py --keys GitignorePromptTitle,GitignorePromptBody

  # Full rewrite of every non-English locale (all strings — slow, overwrites good rows)
  python scripts/render_i18n_locales_from_en.py --full

  # Limit to some locale files (basename without .rs)
  python scripts/render_i18n_locales_from_en.py --locales es_es,ja_jp
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


def should_translate_row(
    key: str,
    en: str,
    cur: str,
    loc_map: dict[str, str],
    *,
    explicit_keys: set[str] | None,
    patch_english: bool,
) -> bool:
    """True when this row should be sent to the translator this run."""
    if explicit_keys is not None:
        return key in explicit_keys
    if key not in loc_map:
        return True
    if patch_english and cur == en:
        return True
    return False


def run_selective_translate(
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

    mode = (
        f"keys={sorted(keys)}"
        if keys
        else ("missing + english-matches" if patch_english else "missing keys only")
    )
    print(f"mode: {mode}")

    total_translated = 0
    total_english_untranslated = 0

    for filename, tgt_code, _label in targets:
        path = LOCALES_DIR / filename
        loc_map = dict(parse_locale_rs(path))
        english_untranslated = [
            k for k, en in en_pairs if k in loc_map and loc_map[k] == en
        ]
        total_english_untranslated += len(english_untranslated)

        translator = GoogleTranslator(source="en", target=tgt_code)
        cache: dict[str, str] = {}
        merged: list[tuple[str, str]] = []
        translated_rows = 0
        for k, en in en_pairs:
            cur = loc_map.get(k, en)
            if should_translate_row(
                k,
                en,
                cur,
                loc_map,
                explicit_keys=keys,
                patch_english=patch_english,
            ):
                cur = translate_cached(translator, cache, en)
                translated_rows += 1
            merged.append((k, cur))
        total_translated += translated_rows

        body = emit_locale_rs(merged)
        previous = path.read_text(encoding="utf-8")
        if body == previous:
            print(
                f"unchanged {path} (0 rows translated; "
                f"{len(english_untranslated)} rows still match English)"
            )
            continue
        path.write_text(body, encoding="utf-8")
        print(
            f"wrote {path} ({translated_rows} rows translated, "
            f"{len(cache)} unique strings in cache)"
        )

    if total_translated == 0:
        print(
            "\nNo rows translated. Default mode only fills I18nKey variants "
            "missing from each locale file."
        )
        if not keys and not patch_english:
            if total_english_untranslated > 0:
                print(
                    "Some locales still use English text copied from en_us.rs "
                    "(rows present but not translated)."
                )
                print(
                    "  • Translate those rows:  --patch-english-matches\n"
                    "  • Or only specific keys: --keys GitignorePromptTitle,…"
                )
            else:
                print("All locale files already contain every key from en_us.rs.")
    print("done")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--full",
        action="store_true",
        help="Re-translate every row in every locale (destructive; not the default).",
    )
    ap.add_argument(
        "--patch-english-matches",
        action="store_true",
        help="Also translate rows that still match English verbatim (in addition to missing keys).",
    )
    ap.add_argument(
        "--keys",
        default="",
        help="Comma-separated I18nKey variant names; only these rows are translated "
        "(ignores missing-key default).",
    )
    ap.add_argument(
        "--locales",
        default="",
        help="Optional comma list of locale basenames (es_es, ja_jp, …). Default: all targets.",
    )
    args = ap.parse_args()
    targets = resolve_targets(args.locales.strip() or None)
    key_set = {x.strip() for x in args.keys.split(",") if x.strip()}

    if args.full:
        if args.patch_english_matches or key_set:
            raise SystemExit("--full cannot be combined with --patch-english-matches or --keys")
        run_full_regen(targets)
        return

    run_selective_translate(
        patch_english=args.patch_english_matches,
        keys=key_set or None,
        targets=targets,
    )


if __name__ == "__main__":
    main()
