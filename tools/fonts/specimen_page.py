"""Write the review page: every CreaBIM font embedded, set in drawing jobs.

  python -I specimen_page.py FONT_DIR OUT.html
"""
import base64, os, sys

FONTS = [
    ("CreaBIM Vectra", "CreaBIM-Vectra", "Single-stroke drafting lettering. The everyday font for notes, tags and dimensions.", ""),
    ("CreaBIM Tech Hand", "CreaBIM-TechHand", "Upright technical handwriting with a felt-tip touch. Notes on hand-line drawings.", ""),
    ("CreaBIM Atelier Hand", "CreaBIM-AtelierHand", "Chisel-nib architect's lettering, slanted. Sketch sheets and presentation notes.", ""),
    ("CreaBIM Chisel", "CreaBIM-Chisel", "Hand lettering traced with a chisel marker.", "Some letters (I, J, X, Y, 6) come out heavier than the rest. That is in Gemini's drawing, not the build."),
    ("CreaBIM Kinetic", "CreaBIM-Kinetic", "Fast, wide callout capitals for titles and callouts.", "The crossbars of A, E, F, H and T run on past the letter. That is Gemini's design, but at small sizes it reads like crossed-out text. Shorten them?"),
    ("CreaBIM Spencer", "CreaBIM-Spencer", "Pointed-nib script for covers and project titles.", ""),
    ("CreaBIM Structa", "CreaBIM-Structa", "Chamfered block capitals for display. Capitals only: lower case types as capitals.", ""),
]


def main(font_dir, out):
    faces, cards = [], []
    for fam, stem, use, note in FONTS:
        path = os.path.join(font_dir, stem + "-Regular.ttf")
        data = open(path, "rb").read()
        b64 = base64.b64encode(data).decode()
        faces.append(f"@font-face{{font-family:'{fam}';src:url(data:font/ttf;base64,{b64}) format('truetype');}}")
        kb = len(data) / 1024
        ff = f"font-family:'{fam}',cursive"
        note_html = f'<p class="note">{note}</p>' if note else ""
        cards.append(f"""<section class="card">
  <header><h2>{fam}</h2><span class="kb">{kb:.0f} KB</span></header>
  <p class="use">{use}</p>
  <div class="spec" style="{ff}">
    <div class="title">Maple Lane House</div>
    <div class="jobs">
      <span class="tag"><b>KITCHEN</b><br><small>19.32 m²</small></span>
      <span class="bub">B</span><span class="sheet">A-101</span>
      <span>12′-6″</span><span>3 810</span><span>Ø20 ±3</span><span>2°</span>
    </div>
    <div class="alpha">ABCDEFGHIJKLMNOPQRSTUVWXYZ<br>abcdefghijklmnopqrstuvwxyz 0123456789</div>
    <div class="txt">Do not scale from this drawing. All dimensions in millimetres unless noted. Façade à vérifier.</div>
  </div>
  {note_html}
</section>""")
    html = f"""<title>CreaBIM Fonts</title>
<meta name="claude:viewport-fit-sides" content="cover">
<style>
{''.join(faces)}
:root{{--bg:#eef0ec;--paper:#fff;--ink:#1d211c;--muted:#5f665d;--rule:#d5d9d2;--accent:#4f6b4c;--mark:#a8552a}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--bg:#171a16;--paper:#20241f;--ink:#e8ebe6;--muted:#a1a89e;--rule:#353a33;--accent:#8fae89;--mark:#e58a5c;color-scheme:dark}}}}
:root[data-theme="dark"]{{--bg:#171a16;--paper:#20241f;--ink:#e8ebe6;--muted:#a1a89e;--rule:#353a33;--accent:#8fae89;--mark:#e58a5c;color-scheme:dark}}
body{{background:var(--bg);color:var(--ink);font:14px/1.5 "Segoe UI",system-ui,sans-serif;margin:0}}
.wrap{{max-width:1100px;margin:0 auto;padding-block:28px 48px;padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px))}}
h1{{font-size:24px;margin:0 0 6px;text-wrap:balance}}
.lede{{color:var(--muted);max-width:72ch;margin:0 0 18px}}
.facts{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,240px),1fr));gap:10px;margin-bottom:22px}}
.facts div{{background:var(--paper);border:1px solid var(--rule);border-radius:6px;padding:10px 12px;font-size:12.5px}}
.facts b{{display:block;font-size:13px}}
.card{{background:var(--paper);border:1px solid var(--rule);border-radius:6px;padding:16px 18px;margin-bottom:14px;min-width:0}}
.card header{{display:flex;justify-content:space-between;align-items:baseline;gap:10px}}
.card h2{{font-size:16px;margin:0}}
.kb{{font-size:11px;color:var(--muted);letter-spacing:.06em}}
.use{{color:var(--muted);font-size:12.5px;margin:2px 0 10px}}
.spec{{overflow-wrap:anywhere}}
.title{{font-size:44px;line-height:1.15;margin-bottom:8px}}
.jobs{{display:flex;flex-wrap:wrap;gap:8px 22px;align-items:center;font-size:22px;margin-bottom:8px}}
.tag{{border:1.3px solid currentColor;padding:2px 10px;text-align:center;line-height:1.2;font-size:16px}}
.tag small{{font-size:12px}}
.bub{{width:30px;height:30px;border:1.3px solid currentColor;border-radius:50%;display:inline-grid;place-items:center;font-size:15px}}
.sheet{{font-size:24px}}
.alpha{{font-size:20px;line-height:1.35;margin-bottom:6px}}
.txt{{font-size:14px}}
.note{{margin:10px 0 0;font-size:12.5px;color:var(--mark)}}
</style>
<div class="wrap">
<h1>CreaBIM's own fonts, REV00</h1>
<p class="lede">The seven letter designs you picked from the Gemini pages, rebuilt as real font files. Every sample on this page is set in those files, so it is exactly what CreaBIM and a PDF would show. Calligra is left out, as you said.</p>
<div class="facts">
 <div><b>Clean outlines</b>Each letter's overlapping pen strokes are merged into one outline, the way a font should be built. Gemini's files were loose stroke pieces.</div>
 <div><b>Drawing marks filled in</b>Missing feet and inch primes (12′-6″), quotes, brackets, °, accents (é è ê à ç) and the dots on i and j are drawn with each font's own pen.</div>
 <div><b>Ours, and light</b>Named CreaBIM, under the SIL Open Font License, 12 to 27 KB each. Built from the saved Gemini pages by a script, with nothing downloaded.</div>
 <div><b>Still to draw</b>? is missing in Atelier Hand, Kinetic, Structa and Vectra; @ in Spencer and Vectra; &amp; in Structa and Vectra.</div>
</div>
{''.join(cards)}
</div>
"""
    open(out, "w", encoding="utf-8").write(html)
    print(out, os.path.getsize(out) // 1024, "KB")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
