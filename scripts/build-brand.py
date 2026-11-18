"""Build Cimori vector masters and Windows exports; does not change application code.

Install fonttools locally into target/brand-tools. Pass --skill-dir pointing at the
installed logo-design skill for its SVG renderer. Exports are deterministic SVGs;
PNGs are rendered separately at every target size, never enlarged from a bitmap.
"""
from pathlib import Path
import argparse
import json
import math
import re
import shutil
import struct
import sys

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'target' / 'brand-tools'))
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('--skill-dir', type=Path, required=True)
args = ap.parse_args()
sys.path.insert(0, str(args.skill_dir / 'scripts'))
import render_png

OUT = REPO / 'assets' / 'brand' / 'cimori'
for folder in ('svg', 'png', 'windows', 'presentation'):
    (OUT / folder).mkdir(parents=True, exist_ok=True)

INK, DARK, LIGHT, BRAND = '#17191D', '#20232A', '#F5F1EB', '#F18B63'
source = (REPO / 'assets/brand/concepts/round-01/a-coroa-v2.svg').read_text(encoding='utf-8')
source = re.sub(r'<title\b[^>]*>.*?</title>', '', source, flags=re.S)
ART = source[source.index('>')+1:source.rindex('</svg>')].strip()


def svg(name, contents, w=256, h=256):
    path = OUT / 'svg' / name
    title = name.replace('.svg', '').replace('-', ' ')
    path.write_text(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:g} {h:g}" role="img" aria-labelledby="title">\n'
                    f'<title id="title">{title}</title>\n{contents}\n</svg>\n', encoding='utf-8')
    return path


def symbol(color, small=False):
    return ART.replace(INK, color).replace('y="24"', 'y="16"' if small else 'y="24"')


masters = {}
for label, color in [('charcoal', INK), ('black', '#000000'), ('white', '#FFFFFF'), ('apricot', BRAND)]:
    masters[label] = svg(f'cimori-symbol-{label}.svg', symbol(color))
    svg(f'cimori-symbol-small-{label}.svg', symbol(color, small=True))

# The wordmark uses the licensed, embedded Inter font at 600 / optical size 32.
# Individual letters are outlined, with deliberately generous optical spacing.
font = instantiateVariableFont(TTFont(REPO / 'crates/isle-render/assets/InterVariable.ttf'), {'wght': 600, 'opsz': 32}, inplace=False)
glyphs = font.getGlyphSet()
cmap = font.getBestCmap()
units = font['head'].unitsPerEm
pen = SVGPathPen(glyphs)
cursor = 0
scale = 176 / units
spacing_after = {'c': 18, 'i': 12, 'm': 10, 'o': 8, 'r': 20}
for i, letter in enumerate('cimori'):
    name = cmap[ord(letter)]
    glyphs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, 24 + cursor * scale, 184)))
    cursor += font['hmtx'][name][0]
    if i < 5:
        cursor += spacing_after[letter]
WORD = re.sub(r'-?\d+\.\d+', lambda m: f'{float(m.group()):.2f}'.rstrip('0').rstrip('.'), pen.getCommands())
WORD_W = math.ceil(cursor * scale + 48)


def word(color):
    return f'<path fill="{color}" d="{WORD}"/>'


for label, color in [('charcoal', INK), ('black', '#000000'), ('white', '#FFFFFF'), ('porcelain', LIGHT)]:
    svg(f'cimori-wordmark-{label}.svg', word(color), WORD_W, 256)

for label, sc, wc in [('light', INK, INK), ('dark', BRAND, LIGHT), ('black', '#000000', '#000000'), ('white', '#FFFFFF', '#FFFFFF')]:
    svg(f'cimori-horizontal-{label}.svg', symbol(sc) + f'<g transform="translate(272 0)">{word(wc)}</g>', 272 + WORD_W, 256)
    x = (max(WORD_W, 256) - 256) / 2
    wx = (max(WORD_W, 256) - WORD_W) / 2
    svg(f'cimori-stacked-{label}.svg', f'<g transform="translate({x:g} 0)">{symbol(sc)}</g>'
        + f'<g transform="translate({wx:g} 190)">{word(wc)}</g>', max(WORD_W, 256), 400)

tile = f'<rect width="256" height="256" rx="56" fill="{DARK}"/>'
icon = svg('cimori-app-icon.svg', tile + symbol(BRAND))
small_icon = svg('cimori-app-icon-small.svg', tile + symbol(BRAND, small=True))


def render(path, dest, w, h):
    used = render_png.render(str(path), str(dest), w, h)
    if not used:
        raise RuntimeError(f'Unable to render {dest}')


sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
ico_pngs = []
for size in sizes + [512, 1024]:
    dest = OUT / 'png' / f'cimori-icon-{size}.png'
    render(small_icon if size <= 32 else icon, dest, size, size)
    if size <= 256:
        ico_pngs.append(str(dest))
render_png.write_ico(ico_pngs, str(OUT / 'windows/cimori.ico'))

for color in ['black', 'white']:
    for size in [16, 20, 24, 32, 40, 48]:
        render(OUT / 'svg' / f'cimori-symbol-small-{color}.svg', OUT / 'windows' / f'cimori-tray-{color}-{size}.png', size, size)

for label in ['light', 'dark', 'black', 'white']:
    w = 1400
    h = round(w * 256 / (272 + WORD_W))
    render(OUT / 'svg' / f'cimori-horizontal-{label}.svg', OUT / 'png' / f'cimori-horizontal-{label}.png', w, h)
render(masters['apricot'], OUT / 'png/cimori-symbol-apricot-1024.png', 1024, 1024)

tokens = {
    'brand': 'Cimori',
    'colors': {'apricot': BRAND, 'graphite': DARK, 'porcelain': LIGHT, 'charcoal': INK, 'mist': '#A7ADB7'},
    'typography': {'family': 'Inter', 'bodyWeights': [400, 500], 'headingWeight': 600, 'wordmark': 'outlined Inter 600, opsz 32, optical spacing'},
    'geometry': {'symbolViewBox': [0, 0, 256, 256], 'clearSpaceUnits': 40, 'iconRadius': 56},
    'usage': {'symbolMinimumPx': 16, 'horizontalMinimumWidthPx': 112, 'stackedMinimumWidthPx': 96, 'smallCutMaxPx': 32},
    'motion': {'durationMs': 240, 'idleAnimation': False, 'respectReducedMotion': True, 'status': 'design guidance, not integrated'}
}
(OUT / 'tokens.json').write_text(json.dumps(tokens, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
shutil.copy2(REPO / 'crates/isle-render/assets/Inter-LICENSE.txt', OUT / 'LICENSE-Inter.txt')
(OUT / 'presentation/fonts').mkdir(exist_ok=True)
shutil.copy2(REPO / 'crates/isle-render/assets/InterVariable.ttf', OUT / 'presentation/fonts/InterVariable.ttf')

# Structural verification of every ICO directory entry and embedded PNG header.
raw = (OUT / 'windows/cimori.ico').read_bytes()
reserved, kind, count = struct.unpack_from('<HHH', raw)
assert (reserved, kind, count) == (0, 1, len(sizes))
for i, expected in enumerate(sizes):
    w, h, palette, zero, planes, bits, length, offset = struct.unpack_from('<BBBBHHII', raw, 6 + i*16)
    w, h = w or 256, h or 256
    assert (w, h, planes, bits) == (expected, expected, 1, 32)
    blob = raw[offset:offset+length]
    assert len(blob) == length and blob[:8] == b'\x89PNG\r\n\x1a\n'
    assert struct.unpack('>II', blob[16:24]) == (expected, expected)
print('ICO validated:', ', '.join(map(str, sizes)))
print('Wrote SVG masters, PNG exports, tray variants, tokens and Inter license to', OUT)
