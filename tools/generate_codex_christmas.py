"""Generate festive original pixel-art overlays for the Codex companion.

Reads the normal Codex sprite sheets, stamps a Santa hat, warm scarf,
blush, a candy cane and tiny winter sparkles into all 24 frames. Preserve
the base animation, transparent backgrounds, and crisp nearest-neighbor
grid for consistent scaling inside the Rust widget.

Run tools/generate_codex_pet.py first, then this script.
"""
from pathlib import Path
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parents[1] / "native" / "assets"
RED = "#d94e56"
RED_SHADE = "#a62f3f"
CREAM = "#fff0d8"
WHITE = "#fffef6"
GREEN = "#267c69"
GOLD = "#f7d07c"
BERRY = "#f2a0a8"

def festive(frame: Image.Image, idx: int, working: bool) -> Image.Image:
    # Sprite assets use 256px cell art with 4px integer pixel blocks.
    tiny = frame.resize((64, 64), Image.Resampling.NEAREST)
    d = ImageDraw.Draw(tiny)
    bob = (idx % 2) if working else 0

    # A tilted, puffed Santa cap drapes over the head and antenna.
    d.polygon([(19, 16), (25, 8), (37, 5), (45, 10), (48, 15),
               (17, 18)], fill=RED_SHADE)
    d.polygon([(19, 15), (27, 8), (37, 6), (44, 10), (46, 14)],
              fill=RED)
    d.rectangle((16, 15, 48, 18), fill=CREAM)
    d.rectangle((17, 16, 46, 17), fill=WHITE)
    d.rectangle((39, 7, 41, 8), fill="#ffb6b9")
    # Plump pompom to the left; tiny glow on alternate animation frames.
    d.ellipse((11, 9, 19, 17), fill=CREAM)
    d.rectangle((14, 10, 17, 12), fill=WHITE)

    # Scarf does not cover the face; knit stripes wrap around the robot's neck.
    d.rectangle((19, 37, 45, 41), fill=RED)
    d.rectangle((22, 38, 42, 39), fill=RED_SHADE)
    d.rectangle((39, 40, 43, 48), fill=RED)
    d.rectangle((40, 43, 43, 44), fill=GOLD)
    d.rectangle((39, 46, 43, 47), fill=CREAM)

    # Rosy cheeks, modest enough to be visible at 60px display width.
    d.rectangle((21, 30, 23, 33), fill=BERRY)
    d.rectangle((41, 30, 43, 33), fill=BERRY)

    if working:
        # Red-and-white candy cane beside the robot's typing hand.
        x = 10 + (idx % 2)
        d.line([(x, 45+bob), (x, 36+bob), (x+3, 33+bob),
                (x+6, 35+bob)], fill=WHITE, width=2)
        for j in range(3):
            d.rectangle((x-1, 36+bob+3*j, x+1, 37+bob+3*j), fill=RED)
        if idx in (1, 5):
            # Small gold idea sparkle.
            d.line([(53, 21), (53, 27)], fill=GOLD, width=1)
            d.line([(50, 24), (56, 24)], fill=GOLD, width=1)
    else:
        # A wrapped present beside the standing character.
        d.rectangle((50, 45, 58, 54), fill=RED_SHADE)
        d.rectangle((51, 46, 57, 52), fill=RED)
        d.rectangle((54, 45, 55, 53), fill=GOLD)
        d.rectangle((51, 48, 57, 49), fill=GOLD)
        if idx % 4 == 2:
            d.rectangle((49, 21, 51, 22), fill=WHITE)
            d.rectangle((50, 20, 50, 23), fill=WHITE)
    return tiny.resize((256, 256), Image.Resampling.NEAREST)

def make(source: str, target: str, columns: int, rows: int, working: bool) -> None:
    sheet = Image.open(HERE / source).convert("RGBA")
    assert sheet.size == (columns*256, rows*256)
    out = Image.new("RGBA", sheet.size, (0,0,0,0))
    for i in range(columns * rows):
        x, y = (i % columns) * 256, (i // columns) * 256
        cell = sheet.crop((x, y, x+256, y+256))
        out.paste(festive(cell, i, working), (x, y))
    out.save(HERE / target, optimize=True)
    print(f"{target}: {out.size}")

if __name__ == "__main__":
    make("codex-working.png", "codex-christmas-working.png", 4, 2, True)
    make("codex-moods.png", "codex-christmas-moods.png", 4, 4, False)
