"""Rebuild the concept-only visual checks. Python stdlib; SVGs stay editable."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent
INK, DARK, LIGHT, APRICOT = '#17191D', '#20232A', '#F5F1EB', '#F18B63'


def inner(path, color=INK):
    raw = path.read_text(encoding='utf-8')
    raw = re.sub(r'<title\b[^>]*>.*?</title>', '', raw, flags=re.S)
    return raw[raw.index('>') + 1:raw.rindex('</svg>')].replace(INK, color)


def symbol(path, x, y, size, color=INK, transform=''):
    content = inner(path, color)
    return (f'<svg x="{x}" y="{y}" width="{size}" height="{size}" viewBox="0 0 256 256">'
            f'<g transform="{transform}">{content}</g></svg>')


def text(x, y, content, size=18, color=INK):
    return f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}">{content}</text>'


def luminance(h):
    rgb = [int(h[i:i+2], 16) / 255 for i in (1, 3, 5)]
    rgb = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in rgb]
    return sum(v*w for v, w in zip(rgb, (.2126, .7152, .0722)))


def contrast(a, b):
    l1, l2 = sorted((luminance(a), luminance(b)), reverse=True)
    return (l1 + .05) / (l2 + .05)


parts = ['<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="1160" viewBox="0 0 1600 1160">',
         '<rect width="1600" height="1160" fill="#F5F1EB"/>',
         '<g font-family="Segoe UI, sans-serif">',
         text(60, 70, 'Cimori / verificação visual dos estudos', 34),
         text(60, 106, 'Tamanhos em pixels reais da prancha. Reflexão e rotação são testes; não são versões de uso.', 20)]

paths = [ROOT / name for name in ['a-coroa-v2.svg', 'b-fresta-v2.svg', 'c-confluencia-v2.svg']]
for i, (path, label) in enumerate(zip(paths, ['A / Coroa suspensa', 'B / Fresta', 'C / Confluência'])):
    y = 150 + i * 255
    parts += [f'<rect x="60" y="{y}" width="1480" height="230" rx="20" fill="white"/>',
              text(85, y + 36, label, 23), symbol(path, 85, y + 60, 128)]
    for x, size in [(285, 64), (400, 48), (510, 32), (610, 24), (710, 16)]:
        parts += [symbol(path, x, y + 100 - size/2, size), text(x, y + 172, f'{size}px', 16)]
    parts += [f'<rect x="825" y="{y+55}" width="135" height="135" rx="16" fill="{DARK}"/>',
              symbol(path, 839, y + 69, 107, LIGHT), text(825, y + 214, 'Invertido', 16),
              symbol(path, 1040, y + 60, 128, transform='translate(256 0) scale(-1 1)'),
              text(1040, y + 214, 'Espelhado', 16),
              symbol(path, 1280, y + 60, 128, transform='rotate(180 128 128)'),
              text(1280, y + 214, 'Rotação 180°', 16)]

parts += [text(60, 970, 'Pista de cor / proposta, pendente de escolha do símbolo', 25)]
for x, color, label in [(60, APRICOT, 'Damasco / #F18B63'), (430, DARK, 'Grafite / #20232A'), (800, LIGHT, 'Porcelana / #F5F1EB')]:
    parts += [f'<rect x="{x}" y="995" width="320" height="65" rx="12" fill="{color}" stroke="#B7B4B0"/>',
              text(x, 1090, label, 18)]
parts += [text(1175, 1017, f'Damasco / grafite: {contrast(APRICOT, DARK):.2f}:1', 18),
          text(1175, 1051, f'Porcelana / grafite: {contrast(LIGHT, DARK):.2f}:1', 18),
          text(1175, 1085, f'Damasco / porcelana: {contrast(APRICOT, LIGHT):.2f}:1', 18), '</g></svg>']
(ROOT / 'verificacao-v2.svg').write_text('\n'.join(parts), encoding='utf-8')
for a, b in [(APRICOT, DARK), (LIGHT, DARK), (INK, LIGHT), ('#A7ADB7', DARK), (APRICOT, LIGHT)]:
    print(f'{a} / {b}: {contrast(a,b):.2f}:1')
print('wrote', ROOT / 'verificacao-v2.svg')
