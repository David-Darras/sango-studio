"""Makes the logo of Sango Studio: a white coral branch on an indigo square.

Writes app/assets/logo.svg and the pictures app/assets/logo_256.png and app/assets/logo_512.png.
The picture is drawn here with Pillow, with the same shapes as the SVG file (no SVG program is needed).

Run: python tools/make_logo.py
"""

from pathlib import Path
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent.parent / "app" / "assets"
SIZE = 160  # the units of the drawing
INDIGO = (79, 70, 229)
WHITE = (255, 255, 255)
SOFT = (199, 210, 254)

# The branch: lines of 9 units with round ends, and the points at the tips.
LINES = [
    [(80, 130), (80, 70)],
    [(80, 108), (54, 84), (54, 58)],
    [(80, 98), (108, 72), (108, 50)],
    [(80, 70), (80, 40)],
    [(54, 84), (38, 70)],
    [(108, 72), (124, 60)],
]
BASE = [(56, 134), (104, 134)]
DOTS = [((54, 56), 7), ((108, 48), 7), ((80, 38), 7), ((38, 70), 6), ((124, 60), 6)]

SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 160" width="160" height="160">
  <title>Sango Studio</title>
  <rect width="160" height="160" rx="36" fill="#4F46E5"/>
  <g fill="none" stroke="#FFFFFF" stroke-width="9" stroke-linecap="round" stroke-linejoin="round">
    <path d="M80 130V70"/>
    <path d="M80 108L54 84V58"/>
    <path d="M80 98L108 72V50"/>
    <path d="M80 70V40"/>
    <path d="M54 84L38 70"/>
    <path d="M108 72L124 60"/>
  </g>
  <path d="M56 134H104" stroke="#C7D2FE" stroke-width="9" stroke-linecap="round"/>
  <g fill="#C7D2FE">
    <circle cx="54" cy="56" r="7"/>
    <circle cx="108" cy="48" r="7"/>
    <circle cx="80" cy="38" r="7"/>
    <circle cx="38" cy="70" r="6"/>
    <circle cx="124" cy="60" r="6"/>
  </g>
</svg>
"""


def stroke(draw, points, width, fill, scale):
    """A polyline with round ends and round corners."""
    pts = [(x * scale, y * scale) for x, y in points]
    draw.line(pts, fill=fill, width=int(width * scale), joint="curve")
    radius = width * scale / 2
    for x, y in pts:
        draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=fill)


def draw(size):
    """The logo as a picture of size x size pixels, drawn 4 times bigger and then reduced."""
    big = size * 4
    scale = big / SIZE
    image = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw_ = ImageDraw.Draw(image)
    draw_.rounded_rectangle((0, 0, big - 1, big - 1), radius=36 * scale, fill=INDIGO)
    for line in LINES:
        stroke(draw_, line, 9, WHITE, scale)
    stroke(draw_, BASE, 9, SOFT, scale)
    for (x, y), r in DOTS:
        draw_.ellipse(((x - r) * scale, (y - r) * scale, (x + r) * scale, (y + r) * scale), fill=SOFT)
    return image.resize((size, size), Image.LANCZOS)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "logo.svg").write_text(SVG, encoding="utf-8")
    for size in (256, 512):
        draw(size).save(OUT / f"logo_{size}.png")
    print("written in", OUT)


if __name__ == "__main__":
    main()
