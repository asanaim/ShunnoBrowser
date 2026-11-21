# Shunno icon: blue squircle tile, white "zero" ring (শূন্য = zero/void) with an opening and an orbiting dot.
import math, sys
from PIL import Image, ImageDraw, ImageFilter

def squircle_mask(S, n=5.0, pad=0.0):
    # Superellipse |x|^n + |y|^n = 1, rendered at S px (supersampled by caller).
    m = Image.new('L', (S, S), 0); d = ImageDraw.Draw(m)
    r = S/2 - pad; cx = cy = S/2
    pts = []
    for i in range(720):
        t = 2*math.pi*i/720
        c, s = math.cos(t), math.sin(t)
        x = cx + r*math.copysign(abs(c)**(2/n), c)
        y = cy + r*math.copysign(abs(s)**(2/n), s)
        pts.append((x, y))
    d.polygon(pts, fill=255)
    return m

def vgrad(S, top, bottom):
    g = Image.new('RGBA', (S, S))
    px = g.load()
    for y in range(S):
        t = y/(S-1)
        col = tuple(int(top[i]*(1-t)+bottom[i]*t) for i in range(3)) + (255,)
        for x in range(S):
            px[x, y] = col
    return g

def render(size, simple=False):
    SS = 8 if size <= 64 else 4
    S = size*SS
    pad = S*(0.015 if size >= 64 else 0.0)
    tile = vgrad(S, (72, 178, 255), (0, 98, 235))
    mask = squircle_mask(S, 5.0, pad)
    out = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    out.paste(tile, (0, 0), mask)
    # soft top highlight
    hl = Image.new('L', (S, S), 0); hd = ImageDraw.Draw(hl)
    hd.ellipse((-S*0.2, -S*0.75, S*1.2, S*0.45), fill=0)
    hl = Image.composite(hl, Image.new('L', (S, S), 0), mask)
    out = Image.composite(Image.new('RGBA', (S, S), (255, 255, 255, 255)), out, hl.point(lambda v: v)).copy() if False else out
    white = Image.new('RGBA', (S, S), (255, 255, 255, 255))
    out.paste(white, (0, 0), hl)
    d = ImageDraw.Draw(out)
    c = S/2
    R = S*(0.275 if not simple else 0.29)          # ring centre radius
    w = S*(0.118 if not simple else 0.16)         # ring stroke
    gap_mid = -45                                  # opening at top-right (degrees, PIL: 0=3 o'clock, clockwise)
    gap = 0 if simple else 64
    box = (c-R-w/2, c-R-w/2, c+R+w/2, c+R+w/2)
    if gap:
        d.arc(box, start=gap_mid+gap/2, end=gap_mid-gap/2+360, fill=(255, 255, 255, 255), width=int(w))
        # rounded ends
        for ang in (gap_mid+gap/2, gap_mid-gap/2):
            a = math.radians(ang)
            x, y = c+R*math.cos(a), c+R*math.sin(a)
            d.ellipse((x-w/2, y-w/2, x+w/2, y+w/2), fill=(255, 255, 255, 255))
        # orbiting dot in the opening, slightly outside the ring
        a = math.radians(gap_mid); rd = w*0.6; Rd = R
        x, y = c+Rd*math.cos(a), c+Rd*math.sin(a)
        d.ellipse((x-rd, y-rd, x+rd, y+rd), fill=(255, 196, 61, 255))
    else:
        d.ellipse(box, outline=(255, 255, 255, 255), width=int(w))
    return out.resize((size, size), Image.LANCZOS)

sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
imgs = {s: render(s, simple=(s <= 20)) for s in sizes}
imgs[256].save('icon_256.png')
imgs[256].resize((512, 512), Image.LANCZOS)
imgs[sizes[-1]].save(sys.argv[1] if len(sys.argv) > 1 else 'app.ico', sizes=[(s, s) for s in sizes], append_images=[imgs[s] for s in sizes[:-1]])
# preview strip on light and dark taskbar colours
strip = Image.new('RGBA', (900, 320), (243, 243, 243, 255))
dark = Image.new('RGBA', (900, 120), (32, 32, 32, 255))
x = 10
for s in [256, 48, 40, 32, 24, 20, 16]:
    strip.paste(imgs[s], (x, 10), imgs[s]); 
    if s <= 48:
        dark.paste(imgs[s], (x, 30), imgs[s])
    x += s + 22
canvas = Image.new('RGBA', (900, 440), (255, 255, 255, 255)); canvas.paste(strip, (0, 0)); canvas.paste(dark, (0, 320))
canvas.save('preview.png')

# Standard Windows layout: 32-bit DIB entries up to 64 px (readable by every loader), PNG above.
import io, struct
def dib(im):
    w, h = im.size
    px = im.tobytes('raw', 'BGRA')
    rows = [px[y*w*4:(y+1)*w*4] for y in range(h)][::-1]
    xor = b''.join(rows)
    mask_row = ((w + 31)//32)*4
    andm = b'\x00' * (mask_row*h)
    hdr = struct.pack('<IiiHHIIiiII', 40, w, h*2, 1, 32, 0, len(xor)+len(andm), 0, 0, 0, 0)
    return hdr + xor + andm
def png(im):
    b = io.BytesIO(); im.save(b, 'PNG', optimize=True); return b.getvalue()
order = [256, 128, 96, 64, 48, 40, 32, 24, 20, 16]
blobs = [(s, png(imgs[s]) if s > 64 else dib(imgs[s])) for s in order]
head = struct.pack('<HHH', 0, 1, len(blobs))
off = 6 + 16*len(blobs); dirs = b''; data = b''
for s, b in blobs:
    dirs += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(b), off + len(data)); data += b
open(sys.argv[1], 'wb').write(head + dirs + data)
