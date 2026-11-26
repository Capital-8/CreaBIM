"""Trim fonts to the letters CreaBIM draws: Latin for English, French and
Spanish, plus the drawing marks. Variable fonts are first fixed at Regular.

  python -I trim_fonts.py IN_DIR OUT_DIR

Prints each font's size before and after. Hinting is dropped (modern
renderers and PDFs do not use it); layout features (kerning, ligatures,
tabular figures, slashed zero, fractions) are kept for the letters kept.
"""
import glob, io, os, sys

from fontTools.ttLib import TTFont
from fontTools import subset
from fontTools.varLib import instancer

# Basic Latin, Latin-1 (French and Spanish accents, ¿¡ ºª ½ ¼ ¾ ° ± × ÷ µ ² ³ Ø),
# and what French, typography and drawings add on top
TEXT_RANGES = [(0x20, 0x7E), (0xA0, 0xFF)]
EXTRA = [
    0x152, 0x153, 0x178,                     # Œ œ Ÿ
    0x2013, 0x2014, 0x2018, 0x2019, 0x201A, 0x201C, 0x201D, 0x201E,
    0x2022, 0x2026, 0x2030, 0x2039, 0x203A, 0x20AC, 0x2122,  # • … ‰ ‹ › € ™
    0x2032, 0x2033,                          # ′ ″  feet and inches
    0x2104, 0x2205, 0x2206, 0x0394, 0x2300,  # ℄ centre line, ∅ ∆ Δ ⌀ diameter
    0x2190, 0x2191, 0x2192, 0x2193,          # ← ↑ → ↓
    0x2212, 0x2248, 0x2260, 0x2264, 0x2265, 0x221E, 0x2220, 0x22A5, 0x2225,  # − ≈ ≠ ≤ ≥ ∞ ∠ ⊥ ∥
    0x2070, 0x2074, 0x2075, 0x2076, 0x2077, 0x2078, 0x2079,  # superscripts
    0x2153, 0x2154, 0x215B, 0x215C, 0x215D, 0x215E,          # ⅓ ⅔ ⅛ ⅜ ⅝ ⅞
    0x2044,                                  # ⁄ fraction slash
]


def unicodes():
    u = set(EXTRA)
    for a, b in TEXT_RANGES:
        u.update(range(a, b + 1))
    return u


def trim(path, out_dir):
    font = TTFont(path)
    if "fvar" in font:
        axes = {a.axisTag: a.defaultValue for a in font["fvar"].axes}
        if "wght" in axes:
            axes["wght"] = 400
        if "wdth" in axes:
            axes["wdth"] = 100
        font = instancer.instantiateVariableFont(font, axes, updateFontNames=True)
    opts = subset.Options()
    opts.layout_features = ["*"]
    opts.hinting = False
    opts.drop_tables += ["DSIG", "STAT"]
    opts.name_IDs = ["*"]
    opts.name_languages = ["*"]
    opts.notdef_outline = True
    opts.glyph_names = False
    s = subset.Subsetter(opts)
    s.populate(unicodes=unicodes())
    s.subset(font)
    if font["name"].getDebugName(1).startswith("Ubuntu"):
        # the Ubuntu Font Licence: a trimmed copy keeps the name and adds
        # "derivative CreaBIM" (UFL 2c)
        for nid, val in ((1, "Ubuntu derivative CreaBIM"), (4, "Ubuntu derivative CreaBIM"),
                         (16, "Ubuntu derivative CreaBIM"), (3, "Ubuntu derivative CreaBIM Regular"),
                         (6, "Ubuntu-derivative-CreaBIM-Regular")):
            font["name"].setName(val, nid, 3, 1, 0x409)
            font["name"].setName(val, nid, 1, 0, 0)
    name = os.path.basename(path)
    for tag in ("[opsz,wght]", "[wdth,wght]", "[wght]"):
        name = name.replace(tag, "-Regular")
    name = name.replace("-lgpl3fe", "-Regular")
    out = os.path.join(out_dir, name)
    font.save(out)
    return name, os.path.getsize(path), os.path.getsize(out)


if __name__ == "__main__":
    src, dst = sys.argv[1], sys.argv[2]
    os.makedirs(dst, exist_ok=True)
    rows = [trim(p, dst) for p in sorted(glob.glob(os.path.join(src, "*.ttf")))]
    for n, a, b in sorted(rows, key=lambda r: r[2]):
        print(f"{n:42s} {a/1024:8.1f} KB -> {b/1024:6.1f} KB {'KEEP' if b <= 100 * 1024 else 'drop'}")
