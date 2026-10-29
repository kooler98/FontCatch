"""Generate the original CC0 CFF font used to verify OpenType -> WOFF2 export.

Requires fontTools 4.60.2. No downloaded or commercial fonts are used.
"""
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.t2CharStringPen import T2CharStringPen

root = Path(__file__).resolve().parents[1] / "src-tauri/tests/fixtures"
builder = FontBuilder(1000, isTTF=False)
glyphs = [".notdef", "space", "A"]
builder.setupGlyphOrder(glyphs)
builder.setupCharacterMap({32: "space", 65: "A"})
builder.setupHorizontalMetrics({name: (600, 0) for name in glyphs})
builder.setupHorizontalHeader(ascent=800, descent=-200)
charstrings = {}
for name in glyphs:
    pen = T2CharStringPen(600, None)
    if name != "space":
        pen.moveTo((100, 0))
        pen.lineTo((300, 700))
        pen.lineTo((500, 0))
        pen.closePath()
    charstrings[name] = pen.getCharString()
builder.setupCFF(
    "FontcatchQA-CFF",
    {"FullName": "Fontcatch QA CFF", "FamilyName": "Fontcatch QA CFF", "Weight": "Regular"},
    charstrings,
    {},
)
builder.setupNameTable({
    "familyName": "Fontcatch QA CFF",
    "styleName": "Regular",
    "uniqueFontIdentifier": "FontcatchQA-CFF",
    "fullName": "Fontcatch QA CFF Regular",
    "psName": "FontcatchQA-CFF",
    "version": "Version 1.000",
    "licenseDescription": "Original Fontcatch test fixture. CC0 1.0 Universal.",
})
builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
builder.setupPost()
builder.setupMaxp()
builder.font.recalcTimestamp = False
builder.font["head"].created = builder.font["head"].modified = 3_876_000_000
builder.save(root / "FontcatchQA-CFF.otf")
