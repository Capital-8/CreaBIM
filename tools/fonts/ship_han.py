"""The three Chinese fonts CreaBIM ships (owner 2026-11-17): cut to GB2312
level 1 plus Latin, punctuation and drawing marks, renamed as their OFL
Reserved Font Names ask of a changed font.

    python.exe -I ship.py <download dir> <assets/fonts/han>
"""
import os, sys
from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

src, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)

chars = set(range(0x20, 0x7F)) | set(range(0xA0, 0x100))
chars |= set(range(0x2010, 0x2070))
chars |= {0x2190, 0x2191, 0x2192, 0x2193, 0x2205, 0x2206, 0x2212, 0x2248, 0x2260, 0x2264, 0x2265, 0x221E, 0x2300, 0x2116, 0x2103}
chars |= set(range(0x3000, 0x3040)) | set(range(0xFF01, 0xFF5F))
for hi in range(0xB0, 0xD8):
    for lo in range(0xA1, 0xFF):
        try:
            chars.add(ord(bytes([hi, lo]).decode("gb2312")))
        except UnicodeDecodeError:
            pass

ship = [
    # (source, file, family, from, copyright line kept)
    ("NotoSansSC-VF.ttf", "CreaBIM-HanSans-Regular.ttf", "CreaBIM Han Sans", "Noto Sans SC"),
    ("Yozai-Regular.ttf", "CreaBIM-HanHand-Regular.ttf", "CreaBIM Han Hand", "Yozai (LXGW, from Y.OzFont)"),
    ("YShiWrittenSC-Regular.ttf", "CreaBIM-HanPen-Regular.ttf", "CreaBIM Han Pen", "Yshi Written (Steve-Yuu, from Zen Kurenaido)"),
]
for name, file, family, origin in ship:
    f = TTFont(os.path.join(src, name))
    if "fvar" in f:
        f = instancer.instantiateVariableFont(f, {"wght": 400})
    opts = subset.Options()
    opts.layout_features = ["*"]
    opts.name_IDs = ["*"]
    opts.notdef_outline = True
    opts.hinting = False
    s = subset.Subsetter(opts)
    s.populate(unicodes=chars)
    s.subset(f)
    # a new name, the old ones gone (OFL: a Modified Version may not use a
    # Reserved Font Name); the copyright and licence records stay
    ps = family.replace(" ", "") + "-Regular"
    names = f["name"]
    keep = {0, 7, 8, 9, 11, 12, 13, 14}
    for rec in list(names.names):
        if rec.nameID not in keep:
            names.removeNames(nameID=rec.nameID)
    for nid, val in [(1, family), (2, "Regular"), (3, f"{ps};CreaBIM"), (4, f"{family} Regular"), (5, "Version 1.000; cut by CreaBIM from " + origin), (6, ps), (16, family), (17, "Regular")]:
        names.setName(val, nid, 3, 1, 0x409)
        names.setName(val, nid, 1, 0, 0)
    # the CFF/post names follow
    f["post"].formatType = 3.0 if "CFF " in f else f["post"].formatType
    p = os.path.join(out, file)
    f.save(p)
    print(f"{file}: {os.path.getsize(p)/1e6:.2f} MB")
