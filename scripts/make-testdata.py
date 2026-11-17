#!/usr/bin/env python3
"""Generate the small test images of testdata/ (IV-01), with the standard library only.

Run from the repository root: python3 scripts/make-testdata.py
The files are committed; tests copy them to a temporary folder and never touch anything else.
Formats that need real encoders (WebP, large photos) come with IV-05.

testdata/orientation-<1..8>.jpg (IV-04): the letter F stored as a camera would store it for each
EXIF Orientation value, plus that tag; shown correctly, every one looks the same — an upright F,
the dot bottom right. The pixels come from testdata/source/f-<n>.jpg (made once with ImageMagick:
the upright F turned by the inverse of each orientation); this script only adds the EXIF segment.
"""
import struct
import zlib
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "testdata"


def png(width, height, rgb):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    row = b"\x00" + bytes(rgb) * width
    raw = row * height
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def bmp(width, height, rgb):
    row = bytes((rgb[2], rgb[1], rgb[0])) * width
    row += b"\x00" * ((4 - len(row) % 4) % 4)
    pixels = row * height
    header = struct.pack("<2sIHHI", b"BM", 54 + len(pixels), 0, 0, 54)
    info = struct.pack("<IiiHHIIiiII", 40, width, height, 1, 24, 0, len(pixels), 2835, 2835, 0, 0)
    return header + info + pixels


def gif(width, height, frames):
    """An animated GIF: one solid colour per frame, 0.5 s each, looping."""
    palette = b"".join(bytes(c) for c in frames) + b"\x00\x00\x00" * (4 - len(frames))
    out = b"GIF89a" + struct.pack("<HHBBB", width, height, 0xF1, 0, 0) + palette
    out += b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"  # loop forever
    for index in range(len(frames)):
        out += b"\x21\xf9\x04\x00" + struct.pack("<H", 50) + b"\x00\x00"
        out += b"\x2c" + struct.pack("<HHHHB", 0, 0, width, height, 0)
        out += b"\x02" + lzw_solid(index, width * height) + b"\x00"
    return out + b"\x3b"


def lzw_solid(color, count):
    """Minimal GIF LZW data (code size 2) for `count` pixels of one colour: clear code before
    every pixel keeps the codes 3 bits wide, which every decoder accepts."""
    clear, end, bits = 4, 5, 3
    codes = []
    for _ in range(count):
        codes += [clear, color]
    codes.append(end)
    acc, n, data = 0, 0, bytearray()
    for code in codes:
        acc |= code << n
        n += bits
        while n >= 8:
            data.append(acc & 0xFF)
            acc >>= 8
            n -= 8
    if n:
        data.append(acc & 0xFF)
    blocks = b"".join(bytes([len(data[i:i + 255])]) + bytes(data[i:i + 255]) for i in range(0, len(data), 255))
    return blocks


def with_orientation(jpeg, code):
    """Insert an EXIF APP1 segment with only the Orientation tag right after SOI."""
    assert jpeg[:2] == b"\xff\xd8"
    tiff = b"II*\x00" + struct.pack("<I", 8) + struct.pack("<H", 1)
    tiff += struct.pack("<HHIHH", 0x0112, 3, 1, code, 0) + struct.pack("<I", 0)
    body = b"Exif\x00\x00" + tiff
    return jpeg[:2] + b"\xff\xe1" + struct.pack(">H", len(body) + 2) + body + jpeg[2:]


def main():
    OUT.mkdir(exist_ok=True)
    files = {
        "red.png": png(64, 48, (200, 40, 40)),
        "blue.bmp": bmp(48, 64, (40, 60, 200)),
        "green.gif": gif(32, 32, [(40, 180, 60), (240, 220, 60)]),
        # Starts like a JPEG but ends after the header: the window shows its error message.
        "broken.jpg": b"\xff\xd8\xff\xe0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00",
        "notes.txt": "Not an image: Spiegelmond refuses to open it.\n".encode(),
    }
    for code in range(1, 9):
        source = OUT / "source" / f"f-{code}.jpg"
        files[f"orientation-{code}.jpg"] = with_orientation(source.read_bytes(), code)
    for name, data in files.items():
        (OUT / name).write_bytes(data)
        print(f"testdata/{name}: {len(data)} bytes")


if __name__ == "__main__":
    main()
