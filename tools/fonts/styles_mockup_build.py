"""Fill the drawing-styles mockup with the reconciled presets and CreaBIM's
lettering fonts. python -I styles_mockup_build.py PRESETS_JSON FONTS_DIR OUT"""
import base64, json, os, sys

src = os.path.join(os.path.dirname(os.path.abspath(__file__)), "styles_mockup_src.html")
presets_json, fonts_dir, out = sys.argv[1:4]
raw = json.load(open(presets_json, encoding="utf-8"))
a, b = raw["hand_line_studio_1.html"], raw["hand_line_studio_2.html"]
full = dict(b["ruled"]["params"])  # the second page has all 22 settings

def norm(p, **extra):
    q = {k: full[k] for k in full}
    for k in q:
        if k in p:
            q[k] = p[k]
    q.setdefault("washOpacity", 0)
    q.update(extra)
    q["inkTone"] = "black"  # every style draws in black ink (owner, MU82)
    return q

presets = {
    "ruled": {"name": "Ruled", "badge": "Clean plotted lines (Gemini: Baseline · CAD)", "params": norm(b["ruled"]["params"])},
    "hand": {"name": "Hand", "badge": "Felt pen on trace (Gemini: Baseline · Studio Felt)", "params": norm(b["hand"]["params"])},
    "loose": {"name": "Loose", "badge": "Ours (REV03): Hand pushed further, for sketches", "params": norm(b["hand"]["params"],
        overshoot=5.2, wobbleAmp=0.42, wobbleWave=50, endAmount=0.85, pressureVar=0.36, doubleOffset=0.45,
        doubleOpacity=0.3, hatchSpacingJitter=0.42, hatchAngleJitter=6.0, hatchBroken=0.4, hatchDensity=0.95,
        letterJitter=0.6, leaderShape="curved", seed=5150)},
    "vellum": {"name": "Vellum Graphite", "badge": "Gemini: New Style I · Beaux-Arts", "params": norm(a["vellum"]["params"])},
    "linen": {"name": "Linen Ink", "badge": "Ruling pen and crowquill (Gemini: New Style II · Linen Ink)", "params": norm(a["linen"]["params"])},
    "marker": {"name": "Marker & Wash", "badge": "Gemini: Studio Wash · Felt Pen", "params": norm(b["marker_wash_wash"]["params"])},
    "field": {"name": "Field Bold", "badge": "Gemini: Field Detail · Bold Cut", "params": norm(b["field_heavy_bold"]["params"])},
    "millwork": {"name": "Millwork Study", "badge": "Desk study on trace, in black ink (Gemini: Crimson Ink · Trace)", "params": norm(b["redline_sketch"]["params"])},
}

faces = []
for fam, stem in (("CreaBIM Vectra", "CreaBIM-Vectra"), ("CreaBIM Tech Hand", "CreaBIM-TechHand"),
                  ("CreaBIM Atelier Hand", "CreaBIM-AtelierHand")):
    data = base64.b64encode(open(os.path.join(fonts_dir, stem + "-Regular.ttf"), "rb").read()).decode()
    faces.append(f"@font-face{{font-family:'{fam}';src:url(data:font/ttf;base64,{data}) format('truetype');}}")

html = open(src, encoding="utf-8").read()
html = html.replace("/*FONTS*/", "".join(faces)).replace("/*PRESETS*/", json.dumps(presets))
open(out, "w", encoding="utf-8").write(html)
print(out, os.path.getsize(out) // 1024, "KB")
