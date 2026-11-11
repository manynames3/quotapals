"""Generate original animated pixel sprites for the Codex companion.

Creates 4x4 moods and 4x2 working PNGs, 256px cells, transparent.
Only requires Pillow; source artwork is defined as geometric pixel primitives.
"""
from pathlib import Path
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parents[1] / "native" / "assets"
OUT = "#182d35"
DARK = "#234c55"
BASE = "#6ab7a2"
LIGHT = "#a5e6c9"
SHADOW = "#418b80"
DEEP = "#143c48"
EYE = "#d3ffe3"
CYAN = "#67ecdc"
GLINT = "#e7fff1"
AMBER = "#ffca66"
FIRE = "#ed7354"
DESK = "#233a42"
DESK_LIGHT = "#4c7677"


def draw_frame(n, state):
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    d = ImageDraw.Draw(image)
    shift = [0, 1, 0, -1][n % 4] if state == "idle" else 0
    x = shift
    # Character shadow / floating hover: one-pixel rhythm
    bob = (n % 2) if state == "working" else 0

    def r(x0, y0, x1, y1, c):
        d.rectangle((x0 + x, y0 + bob, x1 + x, y1 + bob), fill=c)

    # Fire effects behind the body
    if state == "fire":
        phase = n % 4
        for start, alt in [(9, phase), (49, 3-phase)]:
            d.polygon([(start, 46), (start-5, 36), (start-3, 24+alt),
                       (start+1, 29), (start+4, 11-alt*2),
                       (start+9, 29), (start+10, 47)], fill=FIRE)
            d.polygon([(start+1, 43), (start-2, 35), (start+2, 25),
                       (start+7, 37), (start+8, 44)], fill=AMBER)

    # Feet, body, arms: darker outline holds up at tiny sizes
    r(20, 48, 28, 57, OUT)
    r(35, 48, 43, 57, OUT)
    r(22, 50, 27, 54, SHADOW)
    r(36, 50, 41, 54, SHADOW)
    r(18, 35, 46, 53, OUT)
    r(21, 38, 43, 51, SHADOW)
    r(23, 39, 41, 46, BASE)
    # Segmented arms
    arm_offset = n % 2 if state == "working" else 0
    r(11, 38+arm_offset, 21, 49+arm_offset, OUT)
    r(13, 40+arm_offset, 19, 47+arm_offset, BASE)
    r(43, 38+(1-arm_offset), 53, 49+(1-arm_offset), OUT)
    r(45, 40+(1-arm_offset), 51, 47+(1-arm_offset), BASE)

    # Chest: small abstract code symbol, original—not a corporate logo
    r(29, 43, 34, 49, DEEP)
    r(30, 44, 33, 48, CYAN)
    r(28, 45, 29, 47, GLINT)
    r(34, 45, 35, 47, GLINT)

    # Side ears
    r(9, 23, 15, 34, OUT)
    r(11, 25, 16, 31, SHADOW)
    r(49, 23, 55, 34, OUT)
    r(47, 25, 53, 31, SHADOW)
    r(11, 27, 13, 29, CYAN)
    r(51, 27, 53, 29, CYAN)

    # Wide squared-off robot face with pixel-rounded corners
    r(18, 13, 45, 42, OUT)
    r(14, 17, 49, 38, OUT)
    r(17, 16, 47, 39, BASE)
    r(15, 20, 49, 35, BASE)
    r(20, 16, 44, 18, LIGHT)
    r(19, 21, 45, 35, DEEP)
    r(22, 23, 42, 33, DARK)
    r(19, 22, 20, 29, LIGHT)
    # Screen glints
    r(22, 23, 28, 23, "#43787b")
    r(35, 23, 40, 23, "#43787b")
    # Short antenna / indicator
    r(30, 7, 34, 13, OUT)
    r(31, 9, 33, 12, BASE)
    r(29, 4, 35, 10, OUT)
    r(30, 5, 34, 9, CYAN if n % 2 == 0 else LIGHT)

    if state == "sleeping":
        r(23, 28, 28, 29, EYE)
        r(36, 28, 41, 29, EYE)
        # Zzz, move upward slowly
        zy = 10 - (n % 4)
        for xx, yy, s in [(48, zy+9, 4), (54, zy, 5)]:
            r(xx, yy, xx+s, yy, LIGHT)
            r(xx+s-2, yy+1, xx+s-1, yy+2, LIGHT)
            r(xx+1, yy+3, xx+s-2, yy+4, LIGHT)
    elif state == "tired":
        r(23, 27, 28, 28, "#91bbba")
        r(36, 27, 41, 28, "#91bbba")
        r(23, 30, 28, 30, CYAN)
        r(36, 30, 41, 30, CYAN)
        r(29, 34, 35, 34, AMBER)
        r(47, 34, 48, 37, CYAN)
    elif state == "fire":
        r(23, 26, 28, 29, EYE)
        r(36, 26, 41, 29, EYE)
        r(29, 33, 35, 34, AMBER)
    else:
        blink = state == "idle" and n == 2 or state == "working" and n == 4
        if blink:
            r(23, 29, 28, 30, EYE)
            r(36, 29, 41, 30, EYE)
        else:
            r(23, 26, 28, 30, CYAN)
            r(36, 26, 41, 30, CYAN)
            r(24, 26, 25, 27, GLINT)
            r(37, 26, 38, 27, GLINT)
        r(29, 33, 35, 34, EYE)

    # Tiny expressive details: rosy cheeks, antenna sparkles, and an
    # occasional pixel heart. Every mood has its own small personality.
    if state in ("idle", "working"):
        r(20, 32, 22, 34, "#eb9ca4")
        r(42, 32, 44, 34, "#eb9ca4")
    if state == "idle" and n == 3:
        # Happy little pixel heart when there's nothing urgent to do
        r(48, 11, 50, 13, "#f6a3c7")
        r(52, 11, 54, 13, "#f6a3c7")
        r(49, 12, 53, 15, "#f6a3c7")
        r(50, 16, 52, 17, "#f6a3c7")
    if state == "fire" and n % 2 == 1:
        r(46, 31, 47, 33, "#e5faff")  # worried sweat bead
        r(46, 34, 47, 35, "#97c8df")
    if state == "working" and n in (2, 6):
        r(53, 15, 57, 15, GLINT)
        r(55, 13, 55, 17, GLINT)

    # Desk + coding laptop occupy only the working animation
    if state == "working":
        r(6, 52, 58, 58, OUT)
        r(8, 53, 56, 55, DESK_LIGHT)
        r(12, 58, 16, 61, OUT)
        r(48, 58, 52, 61, OUT)
        r(28, 39, 51, 52, OUT)
        r(30, 41, 49, 49, DEEP)
        r(29, 50, 53, 52, LIGHT)
        # animated terminal lines
        for i in range(3):
            width = [12, 8, 14, 10][(n+i) % 4]
            r(32, 42+i*2, 32+width, 42+i*2, CYAN if i == n%3 else BASE)
        r(15, 49-arm_offset, 20, 51-arm_offset, LIGHT)
        if n in (5, 6):
            r(55, 17, 57, 19, AMBER)
            r(58, 14, 59, 17, GLINT)
    return image.resize((256, 256), Image.Resampling.NEAREST)


def sheet(states, filename, columns):
    rows = len(states) // columns
    dst = Image.new("RGBA", (columns * 256, rows * 256), (0, 0, 0, 0))
    for i, state in enumerate(states):
        frame = draw_frame(i if state == "working" else i%4, state)
        dst.paste(frame, ((i%columns)*256, (i//columns)*256))
    dst.save(HERE / filename, optimize=True)
    print(filename, dst.size, (HERE/filename).stat().st_size)


if __name__ == "__main__":
    sheet(["working"]*8, "codex-working.png", 4)
    sheet(["idle"]*4 + ["sleeping"]*4 + ["fire"]*4 + ["tired"]*4, "codex-moods.png", 4)
