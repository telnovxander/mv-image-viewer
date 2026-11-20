#!/usr/bin/env python3
"""Generate the small test images of testdata/ (IV-01), with the standard library only.

Run from the repository root: python3 scripts/make-testdata.py
The files are committed; tests copy them to a temporary folder and never touch anything else.
Formats that need real encoders (WebP, large photos) come with IV-05.

testdata/orientation-<1..8>.jpg (IV-04): the letter F stored as a camera would store it for each
EXIF Orientation value, plus that tag; shown correctly, every one looks the same — an upright F,
the dot bottom right. The pixels come from testdata/source/f-<n>.jpg (made once with ImageMagick:
the upright F turned by the inverse of each orientation); this script only adds the EXIF segment.

IV-05: pages.tif (two pages: 64×40 and 20×20, uncompressed), sample.dng (a TIFF-based RAW
stand-in: Orientation 6 in IFD0, a small JPEG thumbnail and the large JPEG preview in a SubIFD,
taken from testdata/source/f-small.jpg and f-1.jpg), broken.tif and broken.dng.
thumb-6.jpg (IV-08): source/f-6.jpg with Orientation 6 and an EXIF thumbnail (source/f-small.jpg, 40×30)
in IFD1, as cameras store it.
sample.cr3: a Canon CR3 stand-in (ISO BMFF: ftyp "crx ", moov → Canon uuid → CMT1 with Orientation 8,
a uuid box with the PRVW preview from source/f-1.jpg); broken.cr3 ends inside the preview box.
anim.webp (two frames, 0.5 s, looping) and sample.avif are testdata/source/anim.webp and f-1.avif (made
once with Pillow); broken.webp and broken.avif are cut short, the window shows its placeholder.
sample.heic is testdata/source/f-1.heic (made once with libheif: heif-enc -q 40 f-1.jpg), broken.heic
its first 120 bytes (the header boxes without the image data).
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


def with_thumbnail(jpeg, thumb, code):
    """EXIF APP1 with Orientation in IFD0 and a JPEG thumbnail in IFD1 (as cameras write it)."""
    assert jpeg[:2] == b"\xff\xd8"
    ifd0_at = 8
    ifd1_at = ifd0_at + 2 + 12 + 4
    thumb_at = ifd1_at + 2 + 2 * 12 + 4
    tiff = b"II*\x00" + struct.pack("<I", ifd0_at)
    tiff += ifd([tiff_entry(0x0112, 3, 1, code)], ifd1_at)
    tiff += ifd([tiff_entry(0x0201, 4, 1, thumb_at), tiff_entry(0x0202, 4, 1, len(thumb))], 0)
    tiff += thumb
    body = b"Exif\x00\x00" + tiff
    return jpeg[:2] + b"\xff\xe1" + struct.pack(">H", len(body) + 2) + body + jpeg[2:]


def tiff_entry(tag, kind, count, value):
    """One little-endian IFD entry; `value` is the inline value or an offset."""
    if kind == 3 and count == 1:
        return struct.pack("<HHIHH", tag, kind, count, value, 0)
    return struct.pack("<HHII", tag, kind, count, value)


def ifd(entries, next_ifd):
    entries = sorted(entries, key=lambda e: struct.unpack("<H", e[:2])[0])
    return struct.pack("<H", len(entries)) + b"".join(entries) + struct.pack("<I", next_ifd)


def tiff_pages(pages):
    """A little-endian RGB TIFF with one IFD per (width, height, rgb) page, uncompressed."""
    out = bytearray(b"II*\x00" + struct.pack("<I", 0))
    offsets = []
    for width, height, rgb in pages:
        bits = len(out)
        out += struct.pack("<HHH", 8, 8, 8)
        strip = len(out)
        out += bytes(rgb) * (width * height)
        offsets.append((width, height, bits, strip))
    ifd_offsets = []
    for index, (width, height, bits, strip) in enumerate(offsets):
        if len(out) % 2:
            out += b"\x00"
        ifd_offsets.append(len(out))
        entries = [
            tiff_entry(256, 3, 1, width), tiff_entry(257, 3, 1, height), tiff_entry(258, 3, 3, bits),
            tiff_entry(259, 3, 1, 1), tiff_entry(262, 3, 1, 2), tiff_entry(273, 4, 1, strip),
            tiff_entry(277, 3, 1, 3), tiff_entry(278, 3, 1, height), tiff_entry(279, 4, 1, width * height * 3),
        ]
        out += ifd(entries, 0)
    for index, at in enumerate(ifd_offsets):
        next_at = ifd_offsets[index + 1] if index + 1 < len(ifd_offsets) else 0
        count = struct.unpack_from("<H", out, at)[0]
        struct.pack_into("<I", out, at + 2 + count * 12, next_at)
    struct.pack_into("<I", out, 4, ifd_offsets[0])
    return bytes(out)


def raw_stand_in(thumbnail, preview, orientation):
    """A TIFF-based RAW stand-in: IFD0 (Orientation, thumbnail) and a SubIFD with the preview."""
    out = bytearray(b"II*\x00" + struct.pack("<I", 0))
    thumb_at = len(out)
    out += thumbnail
    if len(out) % 2:
        out += b"\x00"
    preview_at = len(out)
    out += preview
    if len(out) % 2:
        out += b"\x00"
    sub_at = len(out)
    out += ifd([tiff_entry(259, 3, 1, 7), tiff_entry(273, 4, 1, preview_at), tiff_entry(279, 4, 1, len(preview))], 0)
    ifd0_at = len(out)
    out += ifd([
        tiff_entry(274, 3, 1, orientation), tiff_entry(330, 4, 1, sub_at),
        tiff_entry(513, 4, 1, thumb_at), tiff_entry(514, 4, 1, len(thumbnail)),
    ], 0)
    struct.pack_into("<I", out, 4, ifd0_at)
    return bytes(out)


def box(kind, payload):
    return struct.pack(">I", 8 + len(payload)) + kind + payload


def cr3_stand_in(jpeg, orientation):
    """The parts of a CR3 Spiegelmond reads: brand, CMT1 (TIFF IFD0) and the PRVW preview."""
    cmt1 = b"II*\x00" + struct.pack("<I", 8) + struct.pack("<H", 1)
    cmt1 += struct.pack("<HHIHH", 0x0112, 3, 1, orientation, 0) + struct.pack("<I", 0)
    canon = bytes.fromhex("85c0b687820f11e08111f4ce462b6a48")
    moov = box(b"moov", box(b"uuid", canon + box(b"CMT1", cmt1)))
    prvw = box(b"PRVW", struct.pack(">IHHHHI", 0, 1, 160, 120, 1, len(jpeg)) + jpeg)
    preview = box(b"uuid", bytes.fromhex("eaf42b5e1c984b88b9fbb7dc406e4d16") + bytes(8) + prvw)
    return box(b"ftyp", b"crx " + struct.pack(">I", 1) + b"crx isom") + moov + preview


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
    files["pages.tif"] = tiff_pages([(64, 40, (200, 120, 40)), (20, 20, (40, 120, 200))])
    files["sample.dng"] = raw_stand_in(
        (OUT / "source" / "f-small.jpg").read_bytes(), (OUT / "source" / "f-1.jpg").read_bytes(), 6
    )
    files["broken.tif"] = files["pages.tif"][:30]
    files["broken.dng"] = b"II*\x00" + struct.pack("<I", 8) + b"\x02\x00garbage"
    files["thumb-6.jpg"] = with_thumbnail(
        (OUT / "source" / "f-6.jpg").read_bytes(), (OUT / "source" / "f-small.jpg").read_bytes(), 6
    )
    files["sample.cr3"] = cr3_stand_in((OUT / "source" / "f-1.jpg").read_bytes(), 8)
    files["broken.cr3"] = files["sample.cr3"][:-500]
    files["anim.webp"] = (OUT / "source" / "anim.webp").read_bytes()
    files["broken.webp"] = files["anim.webp"][:40]
    files["sample.avif"] = (OUT / "source" / "f-1.avif").read_bytes()
    files["broken.avif"] = files["sample.avif"][:60]
    files["sample.heic"] = (OUT / "source" / "f-1.heic").read_bytes()
    files["broken.heic"] = files["sample.heic"][:120]
    for name, data in files.items():
        (OUT / name).write_bytes(data)
        print(f"testdata/{name}: {len(data)} bytes")


if __name__ == "__main__":
    main()
