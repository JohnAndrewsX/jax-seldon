#!/usr/bin/env python3
"""Measure ink in a rectangle of a PNG (bar-view.sh, brief check 4).

Usage: png-ink.py <file.png> <x> <y> <w> <h>

Coordinates are device pixels. The background is the colour of the
rectangle's top-left pixel. A pixel is ink when its largest channel
difference from the background is at least half of the largest difference
any pixel in the rectangle has, so faint anti-aliasing at the edges does
not move the box. Prints JSON: the ink box (top, bottom inclusive, left,
right), its vertical centre in pixel units (rows top..bottom cover
[top, bottom + 1)), the background, and the most frequent ink colour.

Standard library only (zlib), 8-bit RGB/RGBA PNGs without interlacing,
which is what Qt's grabToImage writes.
"""

import json
import struct
import sys
import zlib
from collections import Counter


def read_png(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("not a PNG: " + path)
    pos, idat, width, height, channels = 8, b"", 0, 0, 0
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or colour not in (2, 6) or interlace:
                raise SystemExit("unsupported PNG (depth %d, colour %d, interlace %d)" % (depth, colour, interlace))
            channels = 3 if colour == 2 else 4
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    raw = zlib.decompress(idat)
    stride = width * channels
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(height):
        kind = raw[i]
        line = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - channels] if x >= channels else 0
            b = prev[x]
            c = prev[x - channels] if x >= channels else 0
            if kind == 1:
                line[x] = (line[x] + a) & 255
            elif kind == 2:
                line[x] = (line[x] + b) & 255
            elif kind == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return width, height, channels, rows


def main():
    path, x0, y0, w, h = sys.argv[1], *map(int, sys.argv[2:6])
    width, height, ch, rows = read_png(path)
    px = lambda x, y: tuple(rows[y][x * ch:x * ch + 3])
    box = [(x, y) for y in range(max(0, y0), min(height, y0 + h)) for x in range(max(0, x0), min(width, x0 + w))]
    bg = px(box[0][0], box[0][1])
    diff = lambda c: max(abs(c[i] - bg[i]) for i in range(3))
    peak = max(diff(px(x, y)) for x, y in box)
    ink = [(x, y) for x, y in box if peak > 0 and diff(px(x, y)) * 2 >= peak]
    if not ink:
        print(json.dumps({"ink": False, "background": "#%02x%02x%02x" % bg}))
        return
    top, bottom = min(y for _, y in ink), max(y for _, y in ink)
    left, right = min(x for x, _ in ink), max(x for x, _ in ink)
    colour = Counter(px(x, y) for x, y in ink).most_common(1)[0][0]
    print(json.dumps({
        "ink": True, "top": top, "bottom": bottom, "left": left, "right": right,
        "centre": (top + bottom + 1) / 2, "background": "#%02x%02x%02x" % bg,
        "colour": "#%02x%02x%02x" % colour,
    }))


main()
