# CreaBIM's built-in fonts

Every file is trimmed to Latin for English, French and Spanish, plus the drawing marks (′ ″ ½ ² Ø ± ° ℄ ∅ and arrows); hinting is dropped. Owner's rule (2026-11-16): a Classic ships only if it is 100 KB or less after trimming. The CreaBIM fonts always ship.

## creabim/ (ours, SIL OFL 1.1)

| File | KB | Made from |
|---|---|---|
| CreaBIM-Vectra-Regular.ttf | 11 | Gemini Vectra-45 page |
| CreaBIM-TechHand-Regular.ttf | 21 | Gemini Tech-Draft Hand page |
| CreaBIM-AtelierHand-Regular.ttf | 16 | Gemini Atelier-Hand page |
| CreaBIM-Chisel-Regular.ttf | 17 | Gemini Chisel-Trace page |
| CreaBIM-Kinetic-Regular.ttf | 19 | Gemini Kinetic-Draft page |
| CreaBIM-Spencer-Regular.ttf | 19 | Gemini Spencer-Atelier page |
| CreaBIM-Structa-Regular.ttf | 26 | Gemini Structa-01 page (capitals only) |
| CreaBIM-Symbols-Regular.ttf | 37 | owner's Archi-Glyph 150 page: 150 symbols at U+E000 + kind x 16 + style |

The symbols' native data (what drawings use) is `crates/creabim-core/symbols/creabim-symbols.json`.

## classics/ (trimmed copies)

The 13 fonts are osifont, Inter, IBM Plex Sans, IBM Plex Sans Condensed, IBM Plex Mono, Barlow Condensed, Archivo Narrow, Jost, Noto Sans, Ubuntu, Architects Daughter, Kalam and Patrick Hand.

- **Licences:** OFL 1.1 for all but two: osifont is LGPL 3 with the font exception, and Ubuntu is under the Ubuntu Font Licence. The licence texts are `OFL.txt` and `UFL.txt`.
- **Ubuntu's name:** the trimmed Ubuntu is named "Ubuntu derivative CreaBIM", as the Ubuntu Font Licence (2c) requires.
- **Reserved Font Names:** none of the kept files declares one.
- **Left out:**
  - Source Serif 4: 101 KB once trimmed, and it has the Reserved Font Name "Source".
  - Verdana: its licence does not allow shipping it.

## Rebuilding

The scripts are in `tools/fonts/`. They run on FreeCAD 1.1's Python (fontTools and shapely):

1. `build_creabim_fonts.py`: Gemini raw files to CreaBIM fonts.
2. `build_creabim_symbols.py`: symbol data to the JSON library and the font.
3. `trim_fonts.py`: the trimming.
4. `specimen_png.py` and `fontset_page.py`: check sheets.
