"""List the marks each built font still lacks (shows as a box)."""
import glob, os, sys
from fontTools.ttLib import TTFont

WANT = "?@&()[]\"'éèêàçÉ°Ø±′″"
for p in sorted(glob.glob(os.path.join(sys.argv[1], "*.ttf"))):
    f = TTFont(p)
    c = f.getBestCmap()
    g = f["glyf"]
    miss = [ch for ch in WANT if ord(ch) not in c or g[c[ord(ch)]].numberOfContours == 0]
    print(os.path.basename(p), "".join(miss) or "complete")
