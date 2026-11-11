"""Reproducible original pixel costume collection for QuotaPals.

Generates Halloween, elf, reindeer, winter Codex sprites and a winter
Claude costume, preserving existing sprites' poses and transparency.
Run generate_codex_pet.py before this script. Pillow required.
"""
from pathlib import Path
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parents[1] / "native" / "assets"
SNOW = "#fffaf0"
ICE = "#c5f5fa"
WINTER = "#6ccedb"
WINTER_DARK = "#3b90b1"
RED = "#dd4b59"
GOLD = "#ffd26b"
GREEN = "#30b87c"
GREEN_DEEP = "#167450"
BROWN = "#975f40"
BROWN_DARK = "#5b3626"
PUMPKIN = "#f99936"
VIOLET = "#ba8fe2"

def adorn(image, variant, index, working, claude=False):
    t = image.resize((64, 64), Image.Resampling.NEAREST)
    draw = ImageDraw.Draw(t)
    # Claude base sprite's head is higher and broader than Codex's.
    start = 17 if claude else 17
    right = 47 if claude else 48
    forehead = 18 if claude else 15
    neck = 40 if claude else 38

    if variant == "elf":
        draw.polygon([(start, forehead), (start+6, 10), (start+11, 4), (right+1, 11), (right+1, forehead)], fill=GREEN_DEEP)
        draw.polygon([(start+2, forehead-3), (start+10, 9), (start+12, 6), (right-1, 12), (right-1, forehead-3)], fill=GREEN)
        draw.ellipse((right-2, 10, right+3, 15), fill=GOLD)
        draw.rectangle((start-2,forehead,right+1,forehead+2), fill=SNOW)
        draw.rectangle((start,neck, right,neck+3),fill=GREEN_DEEP)
        draw.rectangle((start+4,neck+1,right-5,neck+2),fill=GREEN)
        draw.rectangle((30,neck+1,34,neck+3),fill=GOLD)
        if not working:
            draw.rectangle((50,45,56,50),fill=RED)
            draw.rectangle((53,44,54,51),fill=GOLD)

    elif variant == "reindeer":
        # Antlers branch above the head with a matching red nose.
        for side in [-1,1]:
            x = 24 if side==-1 else 40
            draw.line([(x,forehead+2),(x+side*3,6)], fill=BROWN_DARK,width=4)
            draw.line([(x+side*3,10),(x+side*9,5)],fill=BROWN,width=3)
            draw.line([(x+side*2,12),(x-side*3,6)],fill=BROWN,width=3)
        draw.rectangle((29,30,35,34),fill=RED)
        draw.rectangle((31,30,33,31),fill="#ffb9c0")
        draw.rectangle((20,neck,right,neck+4),fill=RED)
        draw.rectangle((38,neck+2,42,neck+10),fill=RED)
        draw.ellipse((39,neck+5,44,neck+10),fill=GOLD)

    elif variant == "halloween":
        draw.polygon([(start-4,forehead), (26,12), (30,5), (35,4), (right+3,forehead)],fill="#352343")
        draw.polygon([(start,forehead-2), (29,8), (35,6), (right-1,forehead-3)],fill=VIOLET)
        draw.rectangle((start-4,forehead-2,right+3,forehead+1),fill="#352343")
        draw.rectangle((start+4,forehead,right-4,forehead+1),fill=PUMPKIN)
        draw.ellipse((48,45,60,55),fill=PUMPKIN)
        draw.rectangle((53,43,55,45),fill=GREEN_DEEP)
        draw.rectangle((51,50,53,52),fill=BROWN_DARK)
        draw.rectangle((56,50,58,52),fill=BROWN_DARK)
        draw.rectangle((53,53,56,53),fill=BROWN_DARK)
        if index%4==2:
            draw.line([(52,12),(55,11)],fill=VIOLET,width=2)
            draw.line([(56,10),(58,12)],fill=VIOLET,width=2)

    elif variant == "winter":
        # A fluffy snowman suit around Codex's original friendly robot face.
        if not claude and not working:
            draw.ellipse((18,35,47,59), fill=SNOW)
            draw.ellipse((21,39,44,55), fill=ICE)
            for cy in (46,51):
                draw.ellipse((31,cy,34,cy+3), fill=WINTER_DARK)
        # Cozy blue snow-day hat and puffy scarf, winter stars.
        draw.polygon([(start-2,forehead),(start+2,10),(start+10,6),(right-7,6),(right+3,forehead)],fill=WINTER_DARK)
        draw.polygon([(start+2,forehead-3),(start+8,9),(right-8,8),(right-1,forehead-3)],fill=WINTER)
        draw.rectangle((start-2,forehead-2,right+3,forehead+1),fill=SNOW)
        draw.ellipse((right-1,5,right+5,11),fill=SNOW)
        draw.rectangle((start,neck,right,neck+4),fill=WINTER_DARK)
        draw.rectangle((start+1,neck+1,right-2,neck+2),fill=ICE)
        draw.rectangle((40,neck+3,44,neck+11),fill=WINTER)
        draw.rectangle((41,neck+5,44,neck+6),fill=SNOW)
        if not working:
            for xx,yy in [(54,21),(11,14),(56,47)]:
                draw.line([(xx-3,yy),(xx+3,yy)], fill=ICE, width=1)
                draw.line([(xx,yy-3),(xx,yy+3)], fill=ICE, width=1)
        if index%4==3:
            draw.rectangle((48,32,50,34),fill=SNOW)

    return t.resize((256,256), Image.Resampling.NEAREST)

def sheet(src, dst, rows, variant, claude=False):
    original=Image.open(HERE/src).convert("RGBA")
    cols=4
    assert original.size == (cols*256,rows*256),original.size
    output=Image.new("RGBA",original.size)
    for i in range(cols*rows):
        x=(i%cols)*256;y=(i//cols)*256
        frame=original.crop((x,y,x+256,y+256))
        output.paste(adorn(frame,variant,i,rows==2,claude), (x,y))
    output.save(HERE/dst,optimize=True)
    print(dst,output.size)

if __name__=="__main__":
    for costume in ("elf","reindeer","halloween","winter"):
        sheet("codex-working.png",f"codex-{costume}-working.png",2,costume)
        sheet("codex-moods.png",f"codex-{costume}-moods.png",4,costume)
    sheet("working-sheet.png","claude-winter-working.png",2,"winter",True)
    sheet("moods-sheet.png","claude-winter-moods.png",4,"winter",True)
    sheet("working-sheet.png","claude-elf-working.png",2,"elf",True)
    sheet("moods-sheet.png","claude-elf-moods.png",4,"elf",True)
