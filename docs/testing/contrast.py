#!/usr/bin/env python3
"""WCAG AA contrast checker for the documentation's colour tokens (§8).

Deterministic, no browser. Exits != 0 if any text pair fails AA (4.5:1 normal,
3.0:1 large text / components). Kept in sync with the `CSS` tokens in
`docs/gen.py`; update here when the palette changes.

    python3 docs/testing/contrast.py
"""
import sys


def _lin(c):
    c /= 255
    return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4


def _lum(hexs):
    h = hexs.lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    return 0.2126 * _lin(r) + 0.7152 * _lin(g) + 0.0722 * _lin(b)


def ratio(a, b):
    la, lb = _lum(a), _lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


# (name, fg, bg, threshold). 3.0 = large text/component; 4.5 = normal text.
PAIRS = [
    # Light
    ("light: ink/bg", "#1a1a2e", "#ffffff", 4.5),
    ("light: muted/bg", "#5a6472", "#ffffff", 4.5),
    ("light: muted/side", "#5a6472", "#f7f8fa", 4.5),
    ("light: link/bg", "#b8420a", "#ffffff", 4.5),
    ("light: link/soft (nav+steps)", "#b8420a", "#fff0e6", 4.5),
    ("light: btn/pill white/strong", "#ffffff", "#b8420a", 4.5),
    ("light: out-ink/bg", "#4a5563", "#ffffff", 4.5),
    ("light: tok-comment/code", "#9da7b3", "#0f172a", 4.5),
    ("light: code-head-ink/head", "#cbd5e1", "#1e293b", 4.5),
    # Dark
    ("dark: ink/bg", "#e6e8ee", "#0d1117", 4.5),
    ("dark: muted/bg", "#9aa4b2", "#0d1117", 4.5),
    ("dark: muted/side", "#9aa4b2", "#10151c", 4.5),
    ("dark: link/bg", "#f4733b", "#0d1117", 4.5),
    ("dark: link/soft", "#f4733b", "#2a1810", 4.5),
    ("dark: tok-comment/code", "#8b949e", "#161b22", 4.5),
    ("dark: code-head-ink/head", "#9aa4b2", "#1b222c", 4.5),
]

fails = 0
for name, fg, bg, need in PAIRS:
    r = ratio(fg, bg)
    tag = "PASS" if r >= need else "FAIL"
    if r < need:
        fails += 1
    print(f"  [{tag}] {r:5.2f} (>= {need}) {name}  {fg} on {bg}")

print(f"\n{'OK' if not fails else 'FAILURES: ' + str(fails)} — WCAG AA contrast")
sys.exit(1 if fails else 0)
