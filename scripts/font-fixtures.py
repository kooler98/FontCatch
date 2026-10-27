"""Generate original, tiny test fonts. No third-party fonts or downloads."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont, TTCollection, newTable
from fontTools.ttLib.tables._f_v_a_r import Axis
root=Path(__file__).resolve().parents[1]/'src-tauri/tests/fixtures'
root.mkdir(parents=True,exist_ok=True)
def make(name, family='Fontcatch QA', style='Regular', variable=False, version=1.0):
 b=FontBuilder(1000,isTTF=True);b.setupGlyphOrder(['.notdef','space','A']);b.setupCharacterMap({32:'space',65:'A'})
 glyphs={}
 for g in ['.notdef','space','A']:
  p=TTGlyphPen(None)
  if g!='space':
   p.moveTo((100,0));p.lineTo((300,700));p.lineTo((650 if style == "Bold" else 500,0));p.closePath()
  glyphs[g]=p.glyph()
 b.setupGlyf(glyphs);b.setupHorizontalMetrics({g:(800 if style == 'Bold' else 600,0) for g in glyphs});b.setupHorizontalHeader(ascent=800,descent=-200)
 b.setupNameTable({'familyName':family,'styleName':style,'uniqueFontIdentifier':family+'-'+style,'fullName':family+' '+style,'psName':family.replace(' ','')+'-'+style,'version':f'Version {version:.3f}','manufacturer':'Fontcatch QA','designer':'Fontcatch','licenseDescription':'Original test fixture; unrestricted test use.','licenseInfoURL':'https://example.org/fontcatch-test-license'})
 b.setupOS2(sTypoAscender=800,sTypoDescender=-200,usWinAscent=800,usWinDescent=200);b.setupPost();b.setupMaxp();b.font['head'].fontRevision=version
 if variable:
  fvar=newTable('fvar');a=Axis();a.axisTag='wght';a.minValue=100;a.defaultValue=400;a.maxValue=900;a.flags=0;a.axisNameID=256;fvar.axes=[a];fvar.instances=[];b.font['fvar']=fvar;b.font['name'].setName('Weight',256,3,1,0x409)
 b.save(root/name)
make('FontcatchQA-Regular.ttf')
make('FontcatchQA-Bold.ttf',style='Bold')
make('FontcatchQA-Newer.ttf',version=2.0)
make('FontcatchQA-Variable.ttf',style='Variable',variable=True)
make('FontcatchOther-Regular.ttf',family='Fontcatch Other')
c=TTCollection();c.fonts=[TTFont(root/'FontcatchQA-Regular.ttf'),TTFont(root/'FontcatchOther-Regular.ttf')];c.save(root/'FontcatchQA-Collection.ttc')
(root/'LICENSE.txt').write_text('These synthetic font fixtures were generated for Fontcatch tests. CC0 1.0 Universal.\n')
