"""Write the review page of CreaBIM's built-in fonts and symbols, every
sample set in the shipped (trimmed) files.

  python -I fontset_page.py ASSETS_FONTS_DIR SYMBOLS_JSON OUT.html
"""
import base64, glob, json, os, sys

CREABIM = [
    ("CreaBIM-Vectra", "CreaBIM Vectra", "Single-stroke drafting: notes, tags, dimensions"),
    ("CreaBIM-TechHand", "CreaBIM Tech Hand", "Upright technical handwriting"),
    ("CreaBIM-AtelierHand", "CreaBIM Atelier Hand", "Chisel-nib architect's lettering"),
    ("CreaBIM-Chisel", "CreaBIM Chisel", "Chisel-marker hand lettering"),
    ("CreaBIM-Kinetic", "CreaBIM Kinetic", "Wide callout capitals"),
    ("CreaBIM-Spencer", "CreaBIM Spencer", "Pointed-nib script for covers and titles"),
    ("CreaBIM-Structa", "CreaBIM Structa", "Chamfered display capitals"),
]
CLASSICS = [
    ("osifont-Regular", "osifont", "ISO 3098 drafting lettering", "LGPL 3 + font exception"),
    ("Inter-Regular", "Inter", "CreaBIM's interface font", "OFL"),
    ("IBMPlexSans-Regular", "IBM Plex Sans", "Engineer's sans", "OFL"),
    ("IBMPlexSansCondensed-Regular", "IBM Plex Sans Condensed", "Narrow, for tags and schedules", "OFL"),
    ("IBMPlexMono-Regular", "IBM Plex Mono", "Fixed width, for codes and tables", "OFL"),
    ("BarlowCondensed-Regular", "Barlow Condensed", "Tall narrow sans, for titles", "OFL"),
    ("ArchivoNarrow-Regular", "Archivo Narrow", "Narrow grotesk", "OFL"),
    ("Jost-Regular", "Jost", "Geometric, Futura-like", "OFL"),
    ("NotoSans-Regular", "Noto Sans", "Neutral sans for every language", "OFL"),
    ("Ubuntu-Regular", "Ubuntu derivative CreaBIM", "Ubuntu, trimmed (the licence asks for this name)", "Ubuntu Font Licence"),
    ("ArchitectsDaughter-Regular", "Architects Daughter", "Hand lettering", "OFL"),
    ("Kalam-Regular", "Kalam", "Handwriting", "OFL"),
    ("PatrickHand-Regular", "Patrick Hand", "Handwriting", "OFL"),
]


def face(fam, path):
    b64 = base64.b64encode(open(path, "rb").read()).decode()
    return f"@font-face{{font-family:'{fam}';src:url(data:font/ttf;base64,{b64}) format('truetype');}}"


def card(fam, about, kb, extra=""):
    ff = f"font-family:'{fam}',serif"
    return f"""<section class="card"><header><h3>{fam}</h3><span class="kb">{kb:.0f} KB</span></header>
<p class="use">{about}{extra}</p>
<div style="{ff}"><div class="title">Maple Lane House</div>
<div class="jobs"><span class="tag">KITCHEN<br><small>19.32 m²</small></span><span>12′-6½″</span><span>3 810</span><span>Ø20 ±3</span><span>2°</span><span>A-101</span></div>
<div class="txt">Do not scale from this drawing. Façade à vérifier. ¿Dónde está la cocina?</div></div></section>"""


def main(fonts_dir, sym_json, out):
    faces, rows, total = [], [], 0
    c_cards, k_cards = [], []
    for stem, fam, about in CREABIM:
        p = os.path.join(fonts_dir, "creabim", stem + "-Regular.ttf")
        kb = os.path.getsize(p) / 1024
        total += kb
        faces.append(face(fam, p))
        c_cards.append(card(fam, about, kb))
    sym_path = os.path.join(fonts_dir, "creabim", "CreaBIM-Symbols-Regular.ttf")
    sym_kb = os.path.getsize(sym_path) / 1024
    total += sym_kb
    faces.append(face("CreaBIM Symbols", sym_path))
    classics_kb = 0
    for stem, fam, about, lic in CLASSICS:
        p = os.path.join(fonts_dir, "classics", stem + ".ttf")
        kb = os.path.getsize(p) / 1024
        classics_kb += kb
        faces.append(face(fam, p))
        k_cards.append(card(fam, about, kb, f" · {lic}"))
    total += classics_kb
    lib = json.load(open(sym_json, encoding="utf-8"))
    sym_rows = []
    for ki, k in enumerate(lib["kinds"]):
        cells = "".join(
            f'<div class="sym" title="{s["name"]} ({s["family"]}, {s["char"]})"><span>{chr(0xE000 + ki * 16 + si)}</span><small>{s["family"]}</small></div>'
            for si, s in enumerate(k["styles"]))
        sym_rows.append(f'<div class="kind"><h4>{k["name"]}</h4><div class="syms">{cells}</div></div>')
    creabim_kb = total - classics_kb - sym_kb
    html = f"""<title>CreaBIM Built-in Fonts</title>
<meta name="claude:viewport-fit-sides" content="cover">
<style>
{''.join(faces)}
:root{{--bg:#eef0ec;--paper:#fff;--ink:#1d211c;--muted:#5f665d;--rule:#d5d9d2;--accent:#4f6b4c}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--bg:#171a16;--paper:#20241f;--ink:#e8ebe6;--muted:#a1a89e;--rule:#353a33;--accent:#8fae89;color-scheme:dark}}}}
:root[data-theme="dark"]{{--bg:#171a16;--paper:#20241f;--ink:#e8ebe6;--muted:#a1a89e;--rule:#353a33;--accent:#8fae89;color-scheme:dark}}
body{{background:var(--bg);color:var(--ink);font:14px/1.5 "Segoe UI",system-ui,sans-serif;margin:0}}
.wrap{{max-width:1100px;margin:0 auto;padding-block:28px 48px;padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px))}}
h1{{font-size:24px;margin:0 0 6px;text-wrap:balance}} h2{{font-size:17px;margin:30px 0 4px}}
.lede{{color:var(--muted);max-width:72ch;margin:0 0 16px}}
.sum{{border-collapse:collapse;font-size:13px;margin-bottom:8px}} .sum td{{border-top:1px solid var(--rule);padding:5px 18px 5px 0}} .sum td:last-child{{text-align:right;font-variant-numeric:tabular-nums}}
.sum tr.t td{{font-weight:600;border-top:2px solid var(--ink)}}
.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,500px),1fr));gap:12px}}
.card{{background:var(--paper);border:1px solid var(--rule);border-radius:6px;padding:14px 16px;min-width:0;overflow-wrap:anywhere}}
.card header{{display:flex;justify-content:space-between;gap:10px;align-items:baseline}} .card h3{{font-size:14px;margin:0}}
.kb{{font-size:11px;color:var(--muted)}} .use{{color:var(--muted);font-size:12px;margin:2px 0 8px}}
.title{{font-size:34px;line-height:1.15;margin-bottom:6px}}
.jobs{{display:flex;flex-wrap:wrap;gap:6px 18px;align-items:center;font-size:19px;margin-bottom:6px}}
.tag{{border:1.3px solid currentColor;padding:1px 8px;text-align:center;line-height:1.2;font-size:14px}} .tag small{{font-size:11px}}
.txt{{font-size:14px}}
.kind{{background:var(--paper);border:1px solid var(--rule);border-radius:6px;padding:10px 14px;margin-bottom:8px}}
.kind h4{{margin:0 0 4px;font-size:13px}}
.syms{{display:grid;grid-template-columns:repeat(auto-fill,minmax(72px,1fr));gap:4px}}
.sym{{display:flex;flex-direction:column;align-items:center;padding:4px 0}}
.sym span{{font-family:'CreaBIM Symbols';font-size:46px;line-height:1.1}} .sym small{{font-size:10px;color:var(--muted)}}
</style>
<div class="wrap">
<h1>CreaBIM's built-in fonts</h1>
<p class="lede">The fonts that ship inside CreaBIM: our seven, the symbols and the Classics that stay at 100 KB or under. Each is trimmed to Latin for English, French and Spanish, plus the drawing marks (′ ″ ½ ² Ø ± ° ℄ ∅). Every sample on this page is set in the shipped file.</p>
<table class="sum">
<tr><td>CreaBIM fonts</td><td>7</td><td>{creabim_kb:.0f} KB</td></tr>
<tr><td>CreaBIM Symbols</td><td>150 symbols</td><td>{sym_kb:.0f} KB</td></tr>
<tr><td>Classics</td><td>13</td><td>{classics_kb:.0f} KB</td></tr>
<tr class="t"><td>All</td><td>21 fonts</td><td>{total:.0f} KB</td></tr>
</table>
<p class="lede">Left out: Source Serif 4 (101 KB trimmed, just over the line) and Verdana (its licence does not allow shipping it).</p>
<h2>CreaBIM fonts</h2><div class="grid">{''.join(c_cards)}</div>
<h2>CreaBIM Symbols</h2>
<p class="lede">Your 150 symbols: 15 kinds, ten styles each, from technical to hand-drawn. Hover one for its name.</p>
{''.join(sym_rows)}
<h2>Classics</h2><div class="grid">{''.join(k_cards)}</div>
</div>
"""
    open(out, "w", encoding="utf-8").write(html)
    print(out, os.path.getsize(out) // 1024, "KB;", f"total fonts {total:.0f} KB")


if __name__ == "__main__":
    main(*sys.argv[1:4])
