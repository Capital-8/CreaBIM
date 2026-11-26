"""Build the CreaBIM fonts from the letter designs the owner chose.

The letters were drawn in seven Gemini studio pages (kept in the design folder,
References_Gemini_Structa01_2026-11-16). Each page compiles its own .ttf, but
those files are rough: every stroke is a separate, overlapping, sometimes
self-crossing shape, the names are Gemini's, and some tables are missing.

This script takes those raw files as the letter design and makes a proper font:
  * every glyph's strokes are merged into one clean outline (shapely union),
    wound the TrueType way (outer clockwise, counters anticlockwise);
  * small facets are simplified away (tolerance well under a pixel at any size);
  * missing drawing marks are added from the font's own glyphs: feet and inch
    primes (U+2032, U+2033) from the quotes, and lower case maps to capitals in
    a capitals-only font;
  * all tables are written fresh by fontTools under the CreaBIM name.

Run with a Python that has fontTools and shapely (FreeCAD 1.1's does):
  "C:/Program Files/FreeCAD 1.1/bin/python.exe" -I build_creabim_fonts.py RAW_DIR OUT_DIR
Nothing is downloaded.
"""
import math, os, sys

from fontTools.ttLib import TTFont
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from shapely.geometry import Polygon, MultiPolygon, LineString, Point
from shapely.ops import unary_union
from shapely.validation import make_valid
from shapely.geometry.polygon import orient

VERSION = "0.1"

# raw file -> (family name, file stem, what it is for)
FONTS = {
    "vectra.ttf": ("CreaBIM Vectra", "CreaBIM-Vectra", "Single-stroke drafting lettering for notes, tags and dimensions."),
    "atelier.ttf": ("CreaBIM Atelier Hand", "CreaBIM-AtelierHand", "Architect's chisel-nib hand lettering."),
    "techdraft.ttf": ("CreaBIM Tech Hand", "CreaBIM-TechHand", "Upright technical handwriting with a felt-tip touch."),
    "kinetic.ttf": ("CreaBIM Kinetic", "CreaBIM-Kinetic", "Fast, forward-leaning callout lettering."),
    "chisel.ttf": ("CreaBIM Chisel", "CreaBIM-Chisel", "Hand lettering traced with a chisel marker."),
    "spencer.ttf": ("CreaBIM Spencer", "CreaBIM-Spencer", "Pointed-nib script for covers and titles."),
    "structa.ttf": ("CreaBIM Structa", "CreaBIM-Structa", "Chamfered block capitals for display."),
}

SIMPLIFY = 1.2  # font units (1000 per em): invisible even at 200 pt


def glyph_shape(font, name, dot=None):
    """All contours of a raw glyph merged into one clean shape.

    A contour too small to have an area is a pen dot: it becomes a round dot
    of radius `dot` (when given)."""
    glyf = font["glyf"]
    g = glyf[name]
    if g.numberOfContours <= 0:
        return None
    coords, ends, flags = g.getCoordinates(glyf)
    polys, start = [], 0
    for end in ends:
        pts = [tuple(map(float, coords[i])) for i in range(start, end + 1)]
        start = end + 1
        if len(pts) < 3 or Polygon(pts + pts[:1] * (3 - len(pts))).area < 40:
            if dot:
                xs = [x for x, _ in pts]; ys = [y for _, y in pts]
                polys.append(Point(sum(xs) / len(xs), sum(ys) / len(ys)).buffer(dot, quad_segs=6))
            continue
        p = Polygon(pts)
        if not p.is_valid:
            p = make_valid(p)
        # make_valid can return lines or collections: keep the areas only
        for part in getattr(p, "geoms", [p]):
            if isinstance(part, (Polygon, MultiPolygon)) and part.area > 1.0:
                polys.append(part.buffer(0))
    if not polys:
        return None
    u = unary_union(polys)
    # close hairline slivers between strokes that only touch
    u = u.buffer(0.6, join_style=2).buffer(-0.6, join_style=2)
    return u.simplify(SIMPLIFY, preserve_topology=True)


def draw(shape):
    pen = TTGlyphPen(None)
    if shape is None or shape.is_empty:
        return pen.glyph()
    polys = shape.geoms if hasattr(shape, "geoms") else [shape]
    for poly in polys:
        if not isinstance(poly, Polygon) or poly.area < 2.0:
            continue
        # TrueType: outer contours clockwise, holes anticlockwise (y up)
        poly = orient(poly, sign=-1.0)
        for ring in [poly.exterior, *poly.interiors]:
            pts = [(round(x), round(y)) for x, y in ring.coords[:-1]]
            clean = [p for i, p in enumerate(pts) if p != pts[i - 1]]
            if len(clean) < 3:
                continue
            pen.moveTo(clean[0])
            for p in clean[1:]:
                pen.lineTo(p)
            pen.closePath()
    return pen.glyph()


def notdef(adv, asc):
    w = max(80, int(adv * 0.6))
    outer = Polygon([(50, 0), (50 + w, 0), (50 + w, asc), (50, asc)])
    inner = Polygon([(110, 60), (w - 10, 60), (w - 10, asc - 60), (110, asc - 60)])
    return draw(outer.difference(inner)), w + 100


def paren(stroke, asc, desc, left):
    """( or ) drawn as a stroked arc in the font's own stroke width."""
    top, bot = asc * 0.95, desc * 0.75
    h = top - bot
    pts = []
    for i in range(25):
        t = i / 24
        a = math.radians(-60 + 120 * t)
        x = 260 - math.cos(a) * h * 0.28 * (1 if left else -1) - (0 if left else 120)
        pts.append((x, (top + bot) / 2 + math.sin(a) * h / 2 * 1.0))
    line = LineString(pts)
    if not left:
        line = LineString([(400 - x, y) for x, y in pts])
    return line.buffer(stroke / 2, cap_style=1, join_style=1)


def pen(lines, sw):
    """Polylines drawn with a round pen of width sw, merged."""
    return unary_union([LineString(l).buffer(sw / 2, cap_style=1, join_style=1, quad_segs=6) for l in lines])


def fill_gaps(shapes, advances, cmap, sw, cap_h, x_h, adv):
    """Draw the marks a font lacks with its own pen, so no letter shows as a box.

    Only what is missing is added; a drawn glyph of the page always wins."""
    from shapely import affinity

    def has(ch):
        sh = shapes.get(cmap.get(ord(ch)))
        return sh is not None and not sh.is_empty

    w = adv
    c = w / 2
    q = cap_h * 0.22  # quote length
    marks = {
        "'": [[(c, cap_h), (c, cap_h - q)]],
        '"': [[(c - sw * 1.3, cap_h), (c - sw * 1.3, cap_h - q)], [(c + sw * 1.3, cap_h), (c + sw * 1.3, cap_h - q)]],
        "′": [[(c + q * 0.2, cap_h), (c - q * 0.2, cap_h - q)]],
        "″": [[(c - sw * 1.1 + q * 0.2, cap_h), (c - sw * 1.1 - q * 0.2, cap_h - q)],
                   [(c + sw * 1.4 + q * 0.2, cap_h), (c + sw * 1.4 - q * 0.2, cap_h - q)]],
        "(": [[(c + w * 0.18 - (1 - math.sin(math.radians(a))) * 0 - math.cos(math.radians(a)) * w * 0.28,
                cap_h * 0.45 + math.sin(math.radians(a)) * cap_h * 0.62) for a in range(-70, 71, 5)]],
        "[": [[(c + w * 0.15, cap_h * 1.07), (c - w * 0.12, cap_h * 1.07), (c - w * 0.12, -cap_h * 0.17), (c + w * 0.15, -cap_h * 0.17)]],
        "=": [[(c - w * 0.3, x_h * 0.68), (c + w * 0.3, x_h * 0.68)], [(c - w * 0.3, x_h * 0.32), (c + w * 0.3, x_h * 0.32)]],
        "+": [[(c - w * 0.3, x_h * 0.5), (c + w * 0.3, x_h * 0.5)], [(c, x_h * 0.5 - w * 0.3), (c, x_h * 0.5 + w * 0.3)]],
        "!": [[(c, cap_h), (c, cap_h * 0.3)], [(c, 0), (c, 1)]],
        ":": [[(c, x_h * 0.8), (c, x_h * 0.8 + 1)], [(c, 0), (c, 1)]],
        "°": None,  # ring, below
    }
    for ch, lines in marks.items():
        if has(ch):
            continue
        g = "fill%04X" % ord(ch)
        if ch == "°":
            r = cap_h * 0.12
            shapes[g] = Point(c, cap_h - r).buffer(r + sw / 2).difference(Point(c, cap_h - r).buffer(max(r - sw / 2, 5)))
        else:
            shapes[g] = pen(lines, sw)
        advances[g] = round(w * (0.6 if ch in "'!:([" or ch == "′" else 0.85))
        if ch in "!:'(′[":
            shapes[g] = affinity.translate(shapes[g], -(c - advances[g] / 2))
        cmap[ord(ch)] = g
    # mirrored pairs
    for a, b in (("(", ")"), ("[", "]")):
        if not has(b) and has(a):
            g = "fill%04X" % ord(b)
            src = cmap[ord(a)]
            shapes[g] = affinity.scale(shapes[src], xfact=-1, origin=(advances[src] / 2, 0))
            advances[g] = advances[src]
            cmap[ord(b)] = g
    if not has(";") and has(":") and has(","):
        g = "fill003B"
        colon, comma = shapes[cmap[ord(":")]], shapes[cmap[ord(",")]]
        top = colon.intersection(Polygon([(-500, x_h * 0.4), (2000, x_h * 0.4), (2000, 2000), (-500, 2000)]))
        cx_colon = top.centroid.x
        cx_comma = comma.centroid.x
        shapes[g] = unary_union([top, affinity.translate(comma, cx_colon - cx_comma)])
        advances[g] = advances[cmap[ord(":")]]
        cmap[ord(";")] = g

    # dots on i and j when the page left them off
    for ch in "ij":
        src = cmap.get(ord(ch))
        s = shapes.get(src) if src else None
        if s is None or s.is_empty:
            continue
        if s.bounds[3] < x_h + sw * 1.5:
            stem = s.intersection(Polygon([(-500, x_h * 0.5), (3000, x_h * 0.5), (3000, x_h + 5), (-500, x_h + 5)]))
            cx = stem.centroid.x if not stem.is_empty else s.centroid.x
            shapes[src] = unary_union([s, Point(cx, x_h + sw * 2.2).buffer(sw * 0.62)])

    # accented letters from the base letter and a stroked accent
    def accent(kind, cx, top):
        h, a = sw * 2.4, top + sw * 1.6
        if kind == "acute":
            return pen([[(cx - h * 0.3, a), (cx + h * 0.4, a + h)]], sw * 0.85)
        if kind == "grave":
            return pen([[(cx + h * 0.3, a), (cx - h * 0.4, a + h)]], sw * 0.85)
        if kind == "circ":
            return pen([[(cx - h * 0.6, a), (cx, a + h * 0.8), (cx + h * 0.6, a)]], sw * 0.85)
        if kind == "cedilla":
            return pen([[(cx, 0), (cx, -sw * 1.6), (cx - sw * 1.4, -sw * 2.6)]], sw * 0.85)
    for ch, base, kind in (("é", "e", "acute"), ("è", "e", "grave"), ("ê", "e", "circ"),
                           ("à", "a", "grave"), ("â", "a", "circ"), ("ç", "c", "cedilla"),
                           ("É", "E", "acute"), ("È", "E", "grave"), ("Ç", "C", "cedilla")):
        if has(ch) or not has(base):
            continue
        src = cmap[ord(base)]
        s = shapes.get(src)
        if s is None or s.is_empty:
            continue
        minx, miny, maxx, maxy = s.bounds
        cx = (minx + maxx) / 2
        g = "fill%04X" % ord(ch)
        top = maxy if kind != "cedilla" else 0
        mark = accent(kind, cx, top)
        if base.isupper() and kind != "cedilla":
            # capitals: squeeze the accent into the space above the cap height
            mark = affinity.scale(mark, yfact=0.7, origin=(cx, maxy))
        shapes[g] = unary_union([s, mark])
        advances[g] = advances[src]
        cmap[ord(ch)] = g


def stroke_width(font, cmap):
    """The nib width: 2 x area / perimeter of the hyphen (a capsule), else a stem."""
    g = cmap.get(ord("-"))
    if g:
        s = glyph_shape(font, g)
        if s is not None and s.length > 0:
            return max(30.0, min(140.0, 2 * s.area / s.length))
    for ch in "|l1I":
        gname = cmap.get(ord(ch))
        if not gname:
            continue
        s = glyph_shape(font, gname)
        if s is None:
            continue
        minx, miny, maxx, maxy = s.bounds
        length = max(maxy - miny, 1)
        return max(40.0, min(140.0, s.area / length))
    return 70.0


def build(raw_path, out_dir):
    raw = TTFont(raw_path)
    family, stem, about = FONTS[os.path.basename(raw_path)]
    cmap = dict(raw.getBestCmap() or {})
    hmtx = raw["hmtx"].metrics
    upm = raw["head"].unitsPerEm

    shapes, advances = {}, {}
    sw = stroke_width(raw, cmap)
    for code, gname in cmap.items():
        if gname in shapes:
            continue
        shapes[gname] = glyph_shape(raw, gname, dot=sw * 0.6)
        advances[gname] = hmtx[gname][0]

    ys = [s.bounds for s in shapes.values() if s is not None and not s.is_empty]
    y_max = max(b[3] for b in ys)
    y_min = min(b[1] for b in ys)
    cap = shapes.get(cmap.get(ord("H")))
    cap_h = round(cap.bounds[3]) if cap is not None else round(y_max * 0.9)
    x_shape = shapes.get(cmap.get(ord("x")))
    x_h = round(x_shape.bounds[3]) if x_shape is not None else round(cap_h * 0.7)
    asc = max(round(y_max) + 20, round(upm * 0.8))
    desc = min(round(y_min) - 20, -round(upm * 0.2))

    # gap fills from the font's own glyphs
    adv0 = advances.get(cmap.get(ord("0")), 600)
    caps_only = not any(ord(c) in cmap for c in "abcdefghijklmnopqrstuvwxyz")
    for c in "abcdefghijklmnopqrstuvwxyz":
        if ord(c) not in cmap and ord(c.upper()) in cmap:
            cmap[ord(c)] = cmap[ord(c.upper())]
    fill_gaps(shapes, advances, cmap, sw, cap_h, x_h, adv0)
    if ord(" ") not in cmap:
        shapes["space"] = None
        advances["space"] = round(adv0 * 0.5)
        cmap[ord(" ")] = "space"
    for code, src in ((0x2032, "'"), (0x2033, '"'), (0x00A0, " ")):
        if code not in cmap and ord(src) in cmap:
            cmap[code] = cmap[ord(src)]

    # fresh glyph names, in code order
    names = {}
    order = [".notdef"]
    for code in sorted(cmap):
        src = cmap[code]
        if src not in names:
            n = "uni%04X" % code if code > 0x7F else ("space" if code == 32 else "g%02X" % code)
            names[src] = n
            order.append(n)
    glyphs, metrics = {}, {}
    nd, ndw = notdef(adv0, cap_h)
    glyphs[".notdef"], metrics[".notdef"] = nd, (ndw, 50)
    for src, n in names.items():
        g = draw(shapes.get(src))
        glyphs[n] = g
        metrics[n] = (int(advances.get(src, adv0)), 0)

    fb = FontBuilder(upm, isTTF=True)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap({code: names[src] for code, src in cmap.items()})
    fb.setupGlyf(glyphs)
    # left side bearings from the real outlines
    gl = fb.font["glyf"]
    for n in order:
        g = gl[n]
        g.recalcBounds(gl)
        lsb = getattr(g, "xMin", 0) if g.numberOfContours else 0
        metrics[n] = (metrics[n][0], lsb)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=asc, descent=desc, lineGap=round(upm * 0.08))
    fb.setupNameTable({
        "familyName": family,
        "styleName": "Regular",
        "uniqueFontIdentifier": f"OSAL;{stem};{VERSION}",
        "fullName": family,
        "psName": stem + "-Regular",
        "version": f"Version {VERSION}",
        "manufacturer": "Open Source Architecture Lab",
        "designer": "OSAL with CreaBIM",
        "description": about,
        "copyright": "Copyright 2026 Open Source Architecture Lab (OSAL)",
        "licenseDescription": "Part of CreaBIM. Licensed under the SIL Open Font License 1.1.",
        "licenseInfoURL": "https://openfontlicense.org",
    })
    fb.setupOS2(
        sTypoAscender=asc, sTypoDescender=desc, sTypoLineGap=round(upm * 0.08),
        usWinAscent=asc, usWinDescent=-desc,
        sxHeight=x_h, sCapHeight=cap_h,
        achVendID="OSAL", fsType=0, usWeightClass=400,
        fsSelection=0x40,
    )
    fb.setupPost()
    fb.setupHead(fontRevision=float(VERSION))
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, stem + "-Regular.ttf")
    fb.save(out)
    f = TTFont(out)
    n_pts = sum(len(f["glyf"][n].getCoordinates(f["glyf"])[0]) for n in f.getGlyphOrder() if f["glyf"][n].numberOfContours > 0)
    print(f"{family:24s} {os.path.getsize(out)/1024:6.1f} KB  {len(order):3d} glyphs  {n_pts:5d} points"
          f"  cap {cap_h} x {x_h}  asc {asc} desc {desc}  stroke {sw:.0f}{'  caps only' if caps_only else ''}")
    return out


if __name__ == "__main__":
    raw_dir, out_dir = sys.argv[1], sys.argv[2]
    for raw in FONTS:
        build(os.path.join(raw_dir, raw), out_dir)
