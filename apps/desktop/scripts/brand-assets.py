"""Derive compact letter assets from the canonical Echo wordmark; no font dependency."""
from pathlib import Path
import re
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[1]
source = ET.parse(root / 'design/wordmark/kudzu-wordmark-echo-small.svg').getroot()
letter = []
for node in source:
    if not node.tag.endswith('path'):
        continue
    parts = []
    for part in re.findall(r'M[^M]+', node.attrib['d']):
        numbers = list(map(float, re.findall(r'-?\d+(?:\.\d+)?', part)))
        if max(numbers[::2]) < 155:
            parts.append(part)
    letter.append((node.get('fill-opacity', '1'), parts))

# Optical anchor of the Echo letter's ink, rather than the rectangle around its echoes.
# Fit the printed mark to the top face (above the keycap's front skirt). The slight
# widening toward the viewer follows that face's perspective; this is not a cast shadow.
INK_CENTER = (55.8, 108.63)
FACE_CENTER = (512.0, 442.0)
# Balance the open counterforms and the lighter echoes by eye on the physical face.
OPTICAL_BALANCE_X = -24.0

def project_to_surface(path):
    def point(match):
        x, y = float(match[2]), float(match[3])
        face_y = FACE_CENTER[1] + (y - INK_CENTER[1]) * 2.12
        perspective = 1 + (face_y - FACE_CENTER[1]) * 0.00012
        face_x = FACE_CENTER[0] + (x - INK_CENTER[0]) * 2.34 * perspective + OPTICAL_BALANCE_X
        return f"{match[1]}{face_x:.3f} {face_y:.3f}"
    return re.sub(r'([ML])(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)', point, path)

paths = ''.join(f'<path fill-rule="evenodd" fill-opacity="{opacity}" d="{project_to_surface("".join(parts))}"/>' for opacity, parts in letter)
monogram = f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><g fill="#1447E6">{paths}</g></svg>\n'
(root / 'design/app-icon/layers/02-echo-k-printed.svg').write_text(monogram)
# Import this artwork in Icon Composer. Exporting must not overwrite the native
# document's layer placement, blending, or other adjustments.

# At menu-bar sizes use the filled letter silhouette; the full outlined echo stays in the app icon.
shape = letter[-1][1][-1]
variants = {
    '': '',
    '-paused': '<rect x="11.1" y="10.2" width="1.8" height="5.6" rx=".5"/><rect x="14.3" y="10.2" width="1.8" height="5.6" rx=".5"/>',
    '-permissions': '<path fill-rule="evenodd" d="M14 9A4 4 0 1 1 14 17A4 4 0 1 1 14 9Z M13.4 10.5H14.6V13.6H13.4Z M13.4 14.4H14.6V15.6H13.4Z"/>',
    '-secure-input': '<path d="M11.7 11.5V10C11.7 7 16.1 7 16.1 10V11.5" fill="none" stroke="#000" stroke-width="1.25"/><path fill-rule="evenodd" d="M10.5 11H17.2V16.8H10.5Z M13.3 12.5H14.4V15H13.3Z"/>',
}
for suffix, badge in variants.items():
    mask = '<defs><mask id="badge"><rect width="18" height="18" fill="#fff"/><circle cx="14" cy="13" r="4.8" fill="#000"/></mask></defs>' if badge else ''
    masked = ' mask="url(#badge)"' if badge else ''
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 18 18" fill="#000">{mask}<g{masked}><path transform="translate(3.3 1.3) scale(.085)" d="{shape}"/></g>{badge}</svg>\n'
    (root / f'design/menu-bar/kudzu-menubar{suffix}.svg').write_text(svg)
