from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset
for name, wght, opsz, style in [("twcard-Regular", 430, 14, "Regular"), ("twcard-Heavy", 800, 96, "Bold")]:
    f = TTFont("Bricolage.ttf")
    f = instancer.instantiateVariableFont(f, {"wght": wght, "opsz": opsz, "wdth": 100})
    opts = subset.Options(); opts.layout_features = ["kern", "liga", "tnum", "lnum"]; opts.name_IDs = ["*"]; opts.notdef_outline = True
    s = subset.Subsetter(opts)
    s.populate(unicodes=list(range(0x20, 0x7F)) + list(range(0xA0, 0x100)) + list(range(0x2010, 0x203B)) + [0x2082, 0x2212, 0x2192])
    s.subset(f)
    n = f["name"]
    for rec in list(n.names):
        if rec.nameID in (1, 16): rec.string = "twcard"
        if rec.nameID in (2, 17): rec.string = style
        if rec.nameID in (4,): rec.string = "twcard " + style
        if rec.nameID == 6: rec.string = name
    n.names = [r for r in n.names if r.nameID not in (16, 17, 25)]
    f["OS/2"].usWeightClass = wght if style == "Regular" else 700
    f["OS/2"].fsSelection = (f["OS/2"].fsSelection & ~0b1100001) | (0b100000 if style == "Bold" else 0b1000000)
    f["head"].macStyle = 1 if style == "Bold" else 0
    if "STAT" in f: del f["STAT"]
    f.save(name + ".ttf")
