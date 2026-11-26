"""Build CreaBIM Symbols: the owner's 150 drawing symbols (Archi-Glyph 150).

Input: the symbol data taken from the Archi-Glyph page (archi_glyph.json:
15 kinds x 10 styles, each as strokes, fills, circles, an insertion anchor and
a text slot, on a -16..+15 grid).

Output:
  * creabim-symbols.json: the library CreaBIM draws from natively (curves
    flattened, grid units, y up). Marks are drawn with the view's own pens,
    and the anchor and text slot place them on the drawing.
  * CreaBIM-Symbols-Regular.ttf: the same 150 symbols as a font, at
    U+E000 + kind x 16 + style (private use), for PDFs and other programs.

  "C:/Program Files/FreeCAD 1.1/bin/python.exe" -I build_creabim_symbols.py archi_glyph.json OUT_DIR
"""
import json, os, re, sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from shapely.geometry import LineString, Point, Polygon
from shapely.geometry.polygon import orient
from shapely.ops import unary_union
from shapely.validation import make_valid

VERSION = "0.1"
UNIT = 28.0          # font units per grid step: -16..+15 -> about 870 units
CX, CY = 500.0, 380.0
BASE_HALF = 0.55     # stroke half-width in grid steps at weight 3

KIND_NAMES = {  # CreaBIM's words for each kind
    "arrows": "Leader ends", "points": "Point markers", "sections": "Section marks",
    "elevations": "Elevation marks", "north": "North arrows", "callouts": "Detail callouts",
    "grids": "Grid bubbles", "levels": "Level marks", "tags": "Door and window tags",
    "slopes": "Slope arrows", "revisions": "Revision marks", "centerlines": "Centre and break lines",
    "materials": "Material keys", "mep": "Fixtures and services", "scales": "Scale bars",
}
STYLE_FAMILY = ["Technical", "Technical", "Technical", "Modern", "Modern", "Modern",
                "Conceptual", "Conceptual", "Hand", "Hand"]


def flatten(pts):
    """A stroke's points with quadratic controls (op 2) flattened to a polyline."""
    out = [(pts[0]["x"], pts[0]["y"])]
    i = 1
    while i < len(pts):
        p = pts[i]
        if p.get("op") == 2 and i + 1 < len(pts):
            x0, y0 = out[-1]
            cx, cy = p["x"], p["y"]
            x2, y2 = pts[i + 1]["x"], pts[i + 1]["y"]
            for s in range(1, 9):
                t = s / 8
                m = 1 - t
                out.append((m * m * x0 + 2 * m * t * cx + t * t * x2, m * m * y0 + 2 * m * t * cy + t * t * y2))
            i += 2
        else:
            out.append((p["x"], p["y"]))
            i += 1
    return out


def slug(s):
    return re.sub(r"[^a-z0-9]+", "_", s.lower()).strip("_")


def symbol_shape(sym):
    """One symbol's ink as a merged shape in grid units."""
    parts = []
    g = sym["geom"]
    for st in g["strokes"]:
        pts = flatten(st["pts"])
        if len(pts) < 2:
            continue
        # the stroke's weight: the heaviest of its points' attribute (3 = normal)
        w = max([p.get("a", 3) for p in st["pts"][1:]] or [3]) or 3
        half = BASE_HALF * (0.6 + 0.4 * w / 3)
        parts.append(LineString(pts).buffer(half, cap_style=1, join_style=1, quad_segs=8))
    for f in g["fills"]:
        pts = flatten(f["pts"])
        if len(pts) >= 3:
            p = Polygon(pts)
            parts.append(p if p.is_valid else make_valid(p))
    for c in g["circles"]:
        ring = Point(c["cx"], c["cy"]).buffer(c["r"] + BASE_HALF, quad_segs=24).difference(
            Point(c["cx"], c["cy"]).buffer(max(0.05, c["r"] - BASE_HALF), quad_segs=24))
        parts.append(ring)
    parts = [p.buffer(0) for p in parts if not p.is_empty]
    return unary_union(parts) if parts else None


def draw(shape):
    pen = TTGlyphPen(None)
    if shape is None or shape.is_empty:
        return pen.glyph()
    shape = shape.simplify(0.02, preserve_topology=True)
    for poly in getattr(shape, "geoms", [shape]):
        if not isinstance(poly, Polygon) or poly.area <= 0:
            continue
        poly = orient(poly, sign=-1.0)
        for ring in [poly.exterior, *poly.interiors]:
            pts = [(round(CX + x * UNIT), round(CY + y * UNIT)) for x, y in ring.coords[:-1]]
            pts = [p for i, p in enumerate(pts) if p != pts[i - 1]]
            if len(pts) < 3:
                continue
            pen.moveTo(pts[0])
            for p in pts[1:]:
                pen.lineTo(p)
            pen.closePath()
    return pen.glyph()


def main(src, out_dir):
    data = json.load(open(src, encoding="utf-8"))
    os.makedirs(out_dir, exist_ok=True)
    lib = {"name": "CreaBIM Symbols", "version": VERSION, "units": "grid steps, y up, insertion at the anchor",
           "font": {"family": "CreaBIM Symbols", "first": "U+E000", "code": "U+E000 + kind x 16 + style"},
           "kinds": []}
    order, cmap, glyphs, metrics = [".notdef"], {}, {}, {}
    pen = TTGlyphPen(None)
    for r in ([(100, 0), (800, 0), (800, 700), (100, 700)], [(160, 60), (160, 640), (740, 640), (740, 60)]):
        pen.moveTo(r[0]); [pen.lineTo(p) for p in r[1:]]; pen.closePath()
    glyphs[".notdef"], metrics[".notdef"] = pen.glyph(), (900, 100)
    for ki, cat in enumerate(data["categories"]):
        kind = {"id": cat["id"], "name": KIND_NAMES.get(cat["id"], cat["short"]),
                "gemini_name": cat["name"], "default_text": cat["defaultText"], "styles": []}
        for si, sym in enumerate(cat["symbols"]):
            g = sym["geom"]
            code = 0xE000 + ki * 16 + si
            gname = "uni%04X" % code
            shape = symbol_shape(sym)
            glyphs[gname] = draw(shape)
            metrics[gname] = (1000, 0)
            order.append(gname)
            cmap[code] = gname
            kind["styles"].append({
                "id": f"{cat['id']}.{slug(sym['name'])}",
                "name": sym["name"],
                "family": STYLE_FAMILY[si],
                "char": "U+%04X" % code,
                "strokes": [{"pts": [[round(x, 3), round(y, 3)] for x, y in flatten(st["pts"])],
                             "weight": max([p.get("a", 3) for p in st["pts"][1:]] or [3]) or 3}
                            for st in g["strokes"]],
                "fills": [{"pts": [[round(x, 3), round(y, 3)] for x, y in flatten(f["pts"])], "accent": bool(f.get("accent"))}
                          for f in g["fills"]],
                "circles": [[round(c["cx"], 3), round(c["cy"], 3), c["r"]] for c in g["circles"]],
                "anchor": {"at": [g["anchor"]["x"], g["anchor"]["y"]], "leader_deg": g["anchor"]["dir"] * 45},
                "text": {"at": [g["textSlot"]["x"], g["textSlot"]["y"]], "size": g["textSlot"]["scale"]}
                        if g["textSlot"].get("explicit") else None,
            })
        lib["kinds"].append(kind)

    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glyphs)
    gl = fb.font["glyf"]
    for n in order:
        g = gl[n]
        g.recalcBounds(gl)
        metrics[n] = (metrics[n][0], getattr(g, "xMin", 0) if g.numberOfContours else 0)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=900, descent=-200)
    fb.setupNameTable({
        "familyName": "CreaBIM Symbols", "styleName": "Regular",
        "uniqueFontIdentifier": f"OSAL;CreaBIM-Symbols;{VERSION}", "fullName": "CreaBIM Symbols",
        "psName": "CreaBIM-Symbols-Regular", "version": f"Version {VERSION}",
        "manufacturer": "Open Source Architecture Lab", "designer": "OSAL with CreaBIM",
        "description": "150 architectural drawing symbols: 15 kinds in 10 styles each.",
        "copyright": "Copyright 2026 Open Source Architecture Lab (OSAL)",
        "licenseDescription": "Part of CreaBIM. Licensed under the SIL Open Font License 1.1.",
        "licenseInfoURL": "https://openfontlicense.org",
    })
    fb.setupOS2(sTypoAscender=900, sTypoDescender=-200, usWinAscent=900, usWinDescent=200,
                achVendID="OSAL", fsType=0, fsSelection=0x40)
    fb.setupPost()
    fb.setupHead(fontRevision=float(VERSION))
    ttf = os.path.join(out_dir, "CreaBIM-Symbols-Regular.ttf")
    fb.save(ttf)
    js = os.path.join(out_dir, "creabim-symbols.json")
    with open(js, "w", encoding="utf-8", newline="\n") as f:
        json.dump(lib, f, ensure_ascii=False, separators=(",", ":"))
    print(f"{ttf}: {os.path.getsize(ttf)/1024:.1f} KB, {len(order)-1} symbols")
    print(f"{js}: {os.path.getsize(js)/1024:.1f} KB")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
