"""Render a specimen sheet of the built fonts with FreeType (Pillow), as a check.

  python -I specimen_png.py FONT_DIR OUT.png
"""
import glob, os, sys
from PIL import Image, ImageDraw, ImageFont

font_dir, out = sys.argv[1], sys.argv[2]
files = sorted(glob.glob(os.path.join(font_dir, "*.ttf")))
order = ["Vectra", "AtelierHand", "TechHand", "Kinetic", "Chisel", "Spencer", "Structa"]
files.sort(key=lambda p: next((i for i, k in enumerate(order) if k in p), 99))

W, ROW = 1700, 330
img = Image.new("RGB", (W, ROW * len(files) + 20), "white")
d = ImageDraw.Draw(img)
label = ImageFont.truetype("segoeui.ttf", 18)
y = 10
for p in files:
    name = os.path.basename(p).replace("-Regular.ttf", "").replace("CreaBIM-", "CreaBIM ")
    d.text((20, y), name, fill=(90, 100, 90), font=label)
    big = ImageFont.truetype(p, 64)
    mid = ImageFont.truetype(p, 34)
    small = ImageFont.truetype(p, 20)
    d.text((20, y + 28), "Maple Lane House  Façade", fill="black", font=big)
    d.text((20, y + 118), "ABCDEFGHIJKLMNOPQRSTUVWXYZ  0123456789", fill="black", font=mid)
    d.text((20, y + 165), "abcdefghijklmnopqrstuvwxyz  éèêàç  .,:;-/+=()[]!?&@", fill="black", font=mid)
    d.text((20, y + 215), "KITCHEN 19.32 m   12′-6″   3 810   Ø20 ±3   2°   A-101 (B)", fill="black", font=mid)
    d.text((20, y + 265), "Do not scale from this drawing. All dimensions in millimetres unless noted. Il1 O0", fill="black", font=small)
    d.line((20, y + ROW - 8, W - 20, y + ROW - 8), fill=(220, 224, 218))
    y += ROW
img.save(out)
print(out, img.size)
