#!/usr/bin/env python3
"""Replace hardcoded colors in styles.css with var() refs outside the :root block."""

from __future__ import annotations

from pathlib import Path

import re

# Absolute path keeps cwd-independent behaviour.
_REPO = Path(__file__).resolve().parent.parent
STYLES = _REPO / "styles.css"

# 1-indexed line numbers inclusive; content inside must not be rewritten.
SKIP_LINE_NUMBERS = set(range(6, 19))

# Exact-match replacements as specified (order here is re-sorted below).
_PAIR_SPECS: list[tuple[str, str]] = [
    ("rgba(255, 255, 255, 0.04)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.07)", "var(--overlay-2)"),
    ("rgba(255, 255, 255, 0.1)", "var(--overlay-3)"),
    ("rgba(255, 255, 255, 0.14)", "var(--overlay-4)"),
    ("rgba(255, 255, 255, 0.2)", "var(--overlay-5)"),
    ("rgba(255, 255, 255, 0.28)", "var(--overlay-6)"),
    ("rgba(255, 255, 255, 0.06)", "var(--overlay-2)"),
    ("rgba(255, 255, 255, 0.08)", "var(--overlay-2)"),
    ("rgba(255, 255, 255, 0.05)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.12)", "var(--overlay-3)"),
    ("rgba(255, 255, 255, 0.025)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.03)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.02)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.045)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.035)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.055)", "var(--overlay-2)"),
    ("rgba(255, 255, 255, 0.13)", "var(--overlay-4)"),
    ("rgba(88, 166, 255, 0.16)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.12)", "var(--accent-cool-soft)"),
    ("rgba(88, 166, 255, 0.08)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.075)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.26)", "var(--accent-soft)"),
    ("rgba(91, 157, 255, 0.08)", "var(--accent-soft)"),
    ("#ffffff", "var(--text-bright)"),
    ("#fff", "var(--text-bright)"),
    ("#e57373", "var(--danger)"),
    ("#58a6ff", "var(--accent)"),
    ("#5b9dff", "var(--accent-hover)"),
    ("#7ab8ff", "var(--accent-hover)"),
    ("#4ea1ff", "var(--accent)"),
    ("#8bc4ff", "var(--accent-cool)"),
    ("#2c2f36", "var(--bg-panel-header)"),
    ("#1e2128", "var(--bg-panel)"),
    ("#050608", "var(--bg-app)"),
    ("#17181d", "var(--bg-raised)"),
    ("#101114", "var(--bg-raised)"),
    ("#0e0e10", "var(--on-accent)"),
    ("#4a9eff", "var(--git-lane-0)"),
    ("#a371f7", "var(--git-lane-1)"),
    ("#89ddff", "var(--git-lane-5)"),
    ("#8ec5ff", "var(--accent-cool)"),
    ("rgba(0, 0, 0, 0.45)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.42)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.28)", "var(--scrim-bg)"),
    ("rgba(18, 18, 22, 0.62)", "var(--scrim-bg)"),
    # Pass 2 — extended semantic mappings
    ("rgba(238, 239, 245, 0.95)", "var(--text)"),
    ("rgba(238, 239, 245, 0.92)", "var(--text)"),
    ("rgba(241, 242, 245, 0.55)", "var(--text-muted)"),
    ("rgba(241, 242, 245, 0.72)", "var(--text-muted)"),
    ("rgba(241, 242, 245, 0.9)", "var(--text)"),
    ("rgba(160, 165, 180, 0.7)", "var(--text-muted)"),
    ("rgba(190, 194, 208, 0.78)", "var(--text-muted)"),
    ("rgba(255, 255, 255, 0.09)", "var(--overlay-2)"),
    ("rgba(255, 255, 255, 0.065)", "var(--overlay-2)"),
    ("rgba(255, 255, 255, 0.042)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.015)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.01)", "var(--overlay-1)"),
    ("rgba(255, 255, 255, 0.55)", "var(--overlay-5)"),
    ("rgba(0, 0, 0, 0.25)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.2)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.34)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.32)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.48)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.5)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.58)", "var(--scrim-bg)"),
    ("rgba(0, 0, 0, 0.18)", "var(--scrim-bg)"),
    ("rgba(9, 10, 13, 0.96)", "var(--bg-app)"),
    ("rgba(9, 10, 13, 0.95)", "var(--bg-app)"),
    ("rgba(9, 10, 13, 0.58)", "var(--bg-app)"),
    ("rgba(21, 23, 29, 0.93)", "var(--bg-panel)"),
    ("rgba(12, 13, 18, 0.93)", "var(--bg-raised)"),
    ("rgba(16, 17, 22, 0.74)", "var(--bg-panel)"),
    ("rgba(14, 15, 20, 0.92)", "var(--bg-panel)"),
    ("rgba(18, 19, 24, 0.88)", "var(--bg-panel)"),
    ("rgba(11, 12, 16, 0.9)", "var(--bg-raised)"),
    ("rgba(5, 6, 8, 0.42)", "var(--bg-app)"),
    ("rgba(24, 26, 34, 0.96)", "var(--bg-panel-header)"),
    ("rgba(155, 211, 255, 0.22)", "var(--accent-cool-soft)"),
    ("rgba(155, 211, 255, 0.16)", "var(--accent-cool-soft)"),
    ("rgba(114, 160, 255, 0.9)", "var(--accent)"),
    ("rgba(114, 160, 255, 0.55)", "var(--accent-soft)"),
    ("rgba(114, 160, 255, 0.4)", "var(--accent-soft)"),
    ("rgba(114, 160, 255, 0.18)", "var(--accent-soft)"),
    ("rgba(160, 180, 255, 0.85)", "var(--accent-cool)"),
    ("rgba(120, 160, 255, 0.35)", "var(--accent-soft)"),
    ("rgba(91, 157, 255, 0.12)", "var(--accent-soft)"),
    ("rgba(91, 157, 255, 0.2)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.18)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.32)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.3)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.42)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0.11)", "var(--accent-soft)"),
    ("rgba(88, 166, 255, 0)", "transparent"),
    ("rgba(78, 161, 255, 0.08)", "var(--accent-soft)"),
    ("rgba(99, 230, 190, 0.12)", "var(--accent-cool-soft)"),
    ("rgba(104, 220, 159, 0.08)", "var(--success)"),
    ("rgba(248, 113, 113, 0.08)", "var(--danger-soft)"),
    ("rgba(255, 80, 64, 0.1)", "var(--danger-soft)"),
    ("#e8954a", "var(--agent-claude)"),
    ("#ff9aa2", "var(--danger)"),
    ("#ffb1b1", "var(--danger)"),
    ("#ffb4a8", "var(--danger)"),
    ("#86e7b2", "var(--success)"),
    ("#8fd9c4", "var(--syntax-type)"),
    ("#0d0e12", "var(--on-accent)"),
    ("#000", "var(--bg-app)"),
    ("#0a0a0a", "var(--bg-app)"),
    ("#67e8f9", "var(--syntax-type)"),
    ("#cbd5e1", "var(--text-muted)"),
    ("#fca5a5", "var(--danger)"),
    ("#d1d5db", "var(--text-muted)"),
    ("#fbbf24", "var(--warning)"),
    ("#86efac", "var(--success)"),
    ("#d8e7ff", "var(--accent-cool)"),
    ("#c4a8ff", "var(--syntax-keyword)"),
    ("#9cd2ff", "var(--accent-cool)"),
    ("#ffd166", "var(--warning)"),
    ("#63e6be", "var(--syntax-type)"),
    ("#eaf4ff", "var(--accent-cool-soft)"),
    ("#ffc04d", "var(--warning)"),
    ("#e8b84a", "var(--warning)"),
    ("#ff9a9a", "var(--danger)"),
    ("#9aa", "var(--text-faint)"),
    ("#9aa4b2", "var(--text-muted)"),
    ("#8a8f98", "var(--text-muted)"),
    ("#7dd3fc", "var(--accent-cool)"),
    ("#2a2a2a", "var(--border)"),
]

# Longest needles first avoids e.g. #fff mangling #ffffff.
REPLACEMENTS = sorted(_PAIR_SPECS, key=lambda p: len(p[0]), reverse=True)

_RGBA_RE = re.compile(r"rgba?\([^)]+\)")
_HEX_RE = re.compile(r"#[0-9a-fA-F]{3,8}\b")


def _heuristic_token(literal: str, *, for_text: bool = False) -> str:
    low = literal.lower().replace(" ", "")
    if for_text:
        if low.startswith("rgba(255,255,255") or low.startswith("rgb(255,255,255"):
            # High-alpha white literals are label text, not overlay fills.
            if ",0.9" in low or ",0.92" in low or ",0.95" in low or ",0.88" in low:
                return "var(--text-secondary)"
            if ",0.75" in low or ",0.8" in low or ",0.85" in low:
                return "var(--text-hint)"
            return "var(--text-muted)"
        if low.startswith("rgba(0,0,0") or low.startswith("rgb(0,0,0"):
            return "var(--text-muted)"
        if low.startswith("#"):
            if low in ("#000", "#000000"):
                return "var(--text)"
            return "var(--text-muted)"
        return "var(--text-muted)"
    if low.startswith("rgba(255,255,255") or low.startswith("rgb(255,255,255"):
        return "var(--overlay-2)"
    if low.startswith("rgba(0,0,0") or low.startswith("rgb(0,0,0"):
        return "var(--scrim-bg)"
    if "166,255" in low or "157,255" in low or "144,255" in low or "160,255" in low:
        return "var(--accent-soft)"
    if "230,190" in low or "220,159" in low or "fb950" in low:
        return "var(--success)"
    if "113,113" in low or "248,113" in low or "255,80" in low:
        return "var(--danger-soft)"
    if low.startswith("#"):
        if low in ("#000", "#000000"):
            return "var(--bg-app)"
        return "var(--text-muted)"
    if "9,10,13" in low or "5,6," in low or "12,13,18" in low or "20,22,30" in low:
        return "var(--bg-app)"
    return "var(--overlay-2)"


_TEXT_COLOR_PROPS = frozenset(
    {
        "color",
        "caret-color",
        "text-decoration-color",
        "-webkit-text-fill-color",
    }
)


def _line_sets_text_color(line: str) -> bool:
    if ":" not in line:
        return False
    prop = line.split(":", 1)[0].strip().lower()
    return prop in _TEXT_COLOR_PROPS


def _heuristic_pass(line: str) -> tuple[str, int]:
    total = 0
    for_text = _line_sets_text_color(line)
    out = line

    def repl_rgba(m: re.Match[str]) -> str:
        nonlocal total
        total += 1
        return _heuristic_token(m.group(0), for_text=for_text)

    out = _RGBA_RE.sub(repl_rgba, out)

    def repl_hex(m: re.Match[str]) -> str:
        nonlocal total
        total += 1
        return _heuristic_token(m.group(0), for_text=for_text)

    out = _HEX_RE.sub(repl_hex, out)
    return out, total


def transform_line(line_no: int, line: str) -> tuple[str, int]:
    if line_no in SKIP_LINE_NUMBERS:
        return line, 0
    total = 0
    out = line
    for old, new in REPLACEMENTS:
        cnt = out.count(old)
        if cnt:
            total += cnt
            out = out.replace(old, new)
    out, extra = _heuristic_pass(out)
    total += extra
    return out, total


def main() -> None:
    text = STYLES.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=True)

    rebuilt: list[str] = []
    grand_total = 0
    for i, ln in enumerate(lines, start=1):
        new_ln, cnt = transform_line(i, ln)
        grand_total += cnt
        rebuilt.append(new_ln)

    STYLES.write_text("".join(rebuilt), encoding="utf-8", newline="\n")
    print(f"tokenize_styles_css: {grand_total} replacement(s)")


if __name__ == "__main__":
    main()
