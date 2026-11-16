"""Draws the Folio app icon and the Windows .ico next to it.

The shapes match the logo in the window's top-left corner: an accent-orange
square with rounded corners (radius 9 of 32) and a white Georgia "F" at 20 of
32. Every size is drawn large and scaled down, so the edges stay smooth.

Run from the app folder:  python tools/make_icon.py
"""

from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

ACCENT = (174, 82, 48, 255)
WHITE = (255, 255, 255, 255)
GEORGIA = "C:/Windows/Fonts/georgia.ttf"
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
ASSETS = Path(__file__).resolve().parent.parent / "assets"


def render(size: int) -> Image.Image:
    scale = 8
    big = size * scale
    image = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(
        [0, 0, big - 1, big - 1], radius=round(big * 9 / 32), fill=ACCENT
    )
    font = ImageFont.truetype(GEORGIA, round(big * 20 / 32))
    left, top, right, bottom = draw.textbbox((0, 0), "F", font=font)
    x = (big - (right - left)) / 2 - left
    y = (big - (bottom - top)) / 2 - top
    draw.text((x, y), "F", font=font, fill=WHITE)
    return image.resize((size, size), Image.LANCZOS)


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    frames = [render(size) for size in SIZES]
    frames[-1].save(ASSETS / "icon.png")
    frames[-1].save(
        ASSETS / "icon.ico", format="ICO", sizes=[(s, s) for s in SIZES], append_images=frames
    )
    print("wrote", ASSETS / "icon.png", "and", ASSETS / "icon.ico")


if __name__ == "__main__":
    main()
