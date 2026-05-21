#!/usr/bin/env python3
"""Regenerate src/i18n/locales/{de_de,es_es,fr_fr,...}.rs from en_us.rs using deep-translator.

Requires: pip install deep-translator (use a venv).

Run from repo root:
  # Default: translate only I18nKey rows missing from each locale file (new keys in en_us.rs)
  python scripts/tools/render_i18n_locales_from_en.py

  # Also re-translate rows that still match English verbatim (use sparingly)
  python scripts/tools/render_i18n_locales_from_en.py --patch-english-matches

  # Parallel API calls (e.g. four workers)
  python scripts/tools/render_i18n_locales_from_en.py --patch-english-matches -j 4

  # Only specific keys (comma-separated variant names)
  python scripts/tools/render_i18n_locales_from_en.py --keys GitignorePromptTitle,GitignorePromptBody

  # Full rewrite of every non-English locale (all strings — slow, overwrites good rows)
  python scripts/tools/render_i18n_locales_from_en.py --full

  # Limit to some locale files (basename without .rs)
  python scripts/tools/render_i18n_locales_from_en.py --locales es_es,ja_jp
"""
from __future__ import annotations

import argparse
import re
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from deep_translator import GoogleTranslator
from deep_translator.exceptions import TranslationNotFound

ROOT = Path(__file__).resolve().parents[2]
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


class RateLimiter:
    """Serialize minimum spacing between outbound translate requests."""

    def __init__(self, interval: float = 0.1) -> None:
        self._interval = interval
        self._lock = threading.Lock()
        self._last = 0.0

    def wait(self) -> None:
        with self._lock:
            now = time.monotonic()
            delay = self._interval - (now - self._last)
            if delay > 0:
                time.sleep(delay)
            self._last = time.monotonic()


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


def _unescape_rust_string(s: str) -> str:
    """Reverse the work of `rust_escape`. Critical for round-trip safety:
    `parse → emit → parse → emit` MUST be a fixed point. The previous
    implementation only converted `\\n` to a newline, leaving every other
    escape (`\\\\`, `\\"`, `\\t`, `\\r`) raw — so each parse-emit cycle
    re-escaped the already-escaped backslashes and the file size doubled
    on every `--patch-english-matches` run."""
    out: list[str] = []
    i = 0
    while i < len(s):
        ch = s[i]
        if ch == "\\" and i + 1 < len(s):
            nxt = s[i + 1]
            if nxt == "\\":
                out.append("\\")
                i += 2
                continue
            if nxt == '"':
                out.append('"')
                i += 2
                continue
            if nxt == "n":
                out.append("\n")
                i += 2
                continue
            if nxt == "r":
                out.append("\r")
                i += 2
                continue
            if nxt == "t":
                out.append("\t")
                i += 2
                continue
        out.append(ch)
        i += 1
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
        out.append((key, _unescape_rust_string(s)))
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
        if tr is None:
            raise SystemExit(
                f"emit_locale_rs: translation for I18nKey::{k} is None — "
                "translator returned no result and the fallback didn't kick in"
            )
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


def translate_one(
    translator: GoogleTranslator,
    cache: dict[str, str],
    cache_lock: threading.Lock,
    rate_limiter: RateLimiter | None,
    text: str,
) -> str:
    with cache_lock:
        if text in cache:
            return cache[text]

    for attempt in range(4):
        if rate_limiter is not None:
            rate_limiter.wait()
        else:
            time.sleep(0.1)
        try:
            translated = translator.translate(text)
            # Recent deep-translator versions can return `None` (e.g. when
            # the upstream API responds with no translations for short
            # tokens like "in", "out", "ttft", or a literal glyph like
            # "—"). Treat that as "no translation available" and fall
            # back to the source string so we never cache `None`.
            if not translated:
                with cache_lock:
                    cache[text] = text
                return text
            with cache_lock:
                cache[text] = translated
            return translated
        except TranslationNotFound:
            with cache_lock:
                cache[text] = text
            return text
        except Exception:  # noqa: BLE001 — network flakiness
            if attempt == 3:
                with cache_lock:
                    cache[text] = text
                return text
            time.sleep(0.6 * (attempt + 1))

    with cache_lock:
        cache[text] = text
    return text


def translate_cached(
    translator: GoogleTranslator, cache: dict[str, str], text: str
) -> str:
    return translate_one(translator, cache, threading.Lock(), None, text)


def translate_texts_parallel(
    translator: GoogleTranslator,
    texts: list[str],
    jobs: int,
) -> dict[str, str]:
    """Translate unique strings; returns english -> translation map."""
    unique = list(dict.fromkeys(texts))
    if not unique:
        return {}

    cache: dict[str, str] = {}
    cache_lock = threading.Lock()

    if jobs <= 1:
        for text in unique:
            translate_one(translator, cache, cache_lock, None, text)
        return cache

    rate_limiter = RateLimiter(0.1)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        list(
            pool.map(
                lambda t: translate_one(
                    translator, cache, cache_lock, rate_limiter, t
                ),
                unique,
            )
        )
    return cache


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


def run_full_regen(targets: list[tuple[str, str, str]], jobs: int) -> None:
    pairs = parse_locale_rs(EN_US)
    if len(pairs) < 180:
        raise SystemExit(f"parse too few keys: {len(pairs)}")

    for filename, tgt_code, _label in targets:
        translator = GoogleTranslator(source="en", target=tgt_code)
        en_texts = [en for _, en in pairs]
        cache = translate_texts_parallel(translator, en_texts, jobs)
        merged = [(k, cache[en]) for k, en in pairs]
        path = LOCALES_DIR / filename
        path.write_text(emit_locale_rs(merged), encoding="utf-8")
        print(f"wrote {path} ({len(cache)} unique strings translated, jobs={jobs})")
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
    jobs: int,
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
    print(f"mode: {mode} (jobs={jobs})")

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
        pending_en: list[str] = []
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
                pending_en.append(en)

        cache = translate_texts_parallel(translator, pending_en, jobs)
        translated_rows = len(pending_en)

        merged: list[tuple[str, str]] = []
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
                cur = cache[en]
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
            f"{len(cache)} unique strings in cache, jobs={jobs})"
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
    ap.add_argument(
        "-j",
        "--jobs",
        type=int,
        default=1,
        metavar="N",
        help="Parallel translation workers per locale (default: 1).",
    )
    args = ap.parse_args()

    if args.jobs < 1:
        raise SystemExit("--jobs must be >= 1")

    targets = resolve_targets(args.locales.strip() or None)
    key_set = {x.strip() for x in args.keys.split(",") if x.strip()}

    if args.full:
        if args.patch_english_matches or key_set:
            raise SystemExit("--full cannot be combined with --patch-english-matches or --keys")
        run_full_regen(targets, args.jobs)
        return

    run_selective_translate(
        patch_english=args.patch_english_matches,
        keys=key_set or None,
        targets=targets,
        jobs=args.jobs,
    )


if __name__ == "__main__":
    main()
