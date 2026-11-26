"""Write the Symbols page mockup (Graphic styles > Symbols) with the real
CreaBIM Symbols font and library.

  python -I symbols_mockup.py SYMBOLS_TTF SYMBOLS_JSON OUT.html
"""
import base64, json, os, sys

ttf, js, out = sys.argv[1:4]
lib = json.load(open(js, encoding="utf-8"))
font = base64.b64encode(open(ttf, "rb").read()).decode()

# where each kind lives in CreaBIM today, and the sample text for its slot
USE = {
    "arrows": ("mark", "Note and tag leaders", ""),
    "points": ("new", "Survey points, datum and setting-out marks", ""),
    "sections": ("mark", "Section marks in plans", "1"),
    "elevations": ("mark", "Elevation marks in plans", "2"),
    "north": ("mark", "North arrows on sheets", "N"),
    "callouts": ("mark", "Detail callouts", "3"),
    "grids": ("mark", "Grid bubbles", "B"),
    "levels": ("mark", "Level heads in sections and elevations", "+3.000"),
    "tags": ("mark", "Door and window tags", "D01"),
    "slopes": ("new", "Roof pitch, ramp and fall arrows", "1:12"),
    "revisions": ("mark", "Revision clouds' deltas", "2"),
    "centerlines": ("new", "Centre, symmetry and break lines", "CL"),
    "materials": ("new", "Material keys in sections and details", ""),
    "mep": ("new", "Lights, outlets, sensors in plans", "S"),
    "scales": ("mark", "Scale bars on sheets", "0 1 2 m"),
}
kinds = []
for ki, k in enumerate(lib["kinds"]):
    where, what, sample = USE[k["id"]]
    kinds.append({
        "id": k["id"], "name": k["name"], "where": where, "what": what, "sample": sample,
        "styles": [{"name": s["name"], "family": s["family"], "code": 0xE000 + ki * 16 + si,
                    "text": s["text"]} for si, s in enumerate(k["styles"])],
    })

html = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "symbols_mockup_src.html"), encoding="utf-8").read()
html = html.replace("/*FONT*/", f"@font-face{{font-family:'CreaBIM Symbols';src:url(data:font/ttf;base64,{font}) format('truetype');}}")
html = html.replace("/*KINDS*/", json.dumps(kinds, ensure_ascii=False))
open(out, "w", encoding="utf-8").write(html)
print(out, os.path.getsize(out) // 1024, "KB")
