//! Writing the date taken into a JPEG (IV-14) without disturbing anything else.
//!
//! **Append only.** Nothing of the existing EXIF block moves: a new copy of the Exif IFD (with
//! `DateTimeOriginal` added, changed or removed) and its new value are written after the end of the
//! block, and only the 4-byte pointer to the Exif IFD changes (or, when IFD0 has no Exif IFD yet,
//! IFD0 is copied to the end with that pointer added and the header points to the copy). Every
//! other value keeps its offset, so Maker Notes — which often hold offsets of their own — and all
//! other metadata stay valid byte for byte (point 11). The compressed picture is not touched
//! (point 9). A JPEG without EXIF (WhatsApp and other messengers strip it) gets a small new EXIF
//! block after its JFIF header.

/// EXIF tag numbers used here.
const EXIF_IFD_POINTER: u16 = 0x8769;
const DATE_TIME_ORIGINAL: u16 = 0x9003;
const ASCII: u16 = 2;
const LONG: u16 = 4;

/// `2023-05-12T14:30:00` → the 20 bytes EXIF stores: `2023:05:12 14:30:00\0`.
pub fn exif_date_bytes(iso: &str) -> Option<[u8; 20]> {
    let stamp = super::dates::Stamp::parse(iso)?;
    let text = format!(
        "{:04}:{:02}:{:02} {:02}:{:02}:{:02}",
        stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
    );
    let mut out = [0u8; 20];
    out[..19].copy_from_slice(text.as_bytes());
    Some(out)
}

struct Tiff<'a> {
    data: &'a [u8],
    little: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.data.get(at..at + 2)?;
        Some(if self.little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }
    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.data.get(at..at + 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn put_u16(&self, out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&if self.little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        });
    }
    fn put_u32(&self, out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&if self.little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        });
    }
    fn u32_bytes(&self, v: u32) -> [u8; 4] {
        if self.little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }
    /// The raw 12-byte entries of the IFD at `at` and its next-IFD pointer.
    fn ifd(&self, at: usize) -> Option<(Vec<[u8; 12]>, u32)> {
        let count = usize::from(self.u16(at)?);
        if count > 1000 {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let start = at + 2 + i * 12;
            let raw: [u8; 12] = self.data.get(start..start + 12)?.try_into().ok()?;
            entries.push(raw);
        }
        let next = self.u32(at + 2 + count * 12)?;
        Some((entries, next))
    }
    fn tag(&self, entry: &[u8; 12]) -> u16 {
        if self.little {
            u16::from_le_bytes([entry[0], entry[1]])
        } else {
            u16::from_be_bytes([entry[0], entry[1]])
        }
    }
}

/// Where the EXIF APP1 segment of a JPEG is: (segment start, payload start, payload end).
fn find_exif(jpeg: &[u8]) -> Option<(usize, usize, usize)> {
    let mut at = 2;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF {
        let marker = jpeg[at + 1];
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
        if length < 2 || at + 2 + length > jpeg.len() {
            return None;
        }
        if marker == 0xE1 && jpeg.get(at + 4..at + 10) == Some(&b"Exif\0\0"[..]) {
            return Some((at, at + 4, at + 2 + length));
        }
        at += 2 + length;
    }
    None
}

/// Where a new APP1 goes: after SOI and a JFIF APP0, if there is one.
fn insert_point(jpeg: &[u8]) -> usize {
    if jpeg.get(2..4) == Some(&[0xFF, 0xE0][..]) {
        if let Some(b) = jpeg.get(4..6) {
            return 4 + usize::from(u16::from_be_bytes([b[0], b[1]]));
        }
    }
    2
}

/// An APP1 segment holding `tiff`.
fn app1(tiff: &[u8]) -> Option<Vec<u8>> {
    let length = u16::try_from(tiff.len() + 8).ok()?;
    let mut out = vec![0xFF, 0xE1];
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(tiff);
    Some(out)
}

/// A minimal EXIF block: IFD0 with only the Exif pointer, an Exif IFD with only the date.
fn new_tiff(date: &[u8; 20]) -> Vec<u8> {
    let t = Tiff {
        data: &[],
        little: true,
    };
    let mut out = b"II*\0".to_vec();
    t.put_u32(&mut out, 8);
    // IFD0 at 8: one entry, next 0 → 18 bytes; Exif IFD at 26: one entry → 18 bytes; date at 44.
    t.put_u16(&mut out, 1);
    t.put_u16(&mut out, EXIF_IFD_POINTER);
    t.put_u16(&mut out, LONG);
    t.put_u32(&mut out, 1);
    t.put_u32(&mut out, 26);
    t.put_u32(&mut out, 0);
    t.put_u16(&mut out, 1);
    t.put_u16(&mut out, DATE_TIME_ORIGINAL);
    t.put_u16(&mut out, ASCII);
    t.put_u32(&mut out, 20);
    t.put_u32(&mut out, 44);
    t.put_u32(&mut out, 0);
    out.extend_from_slice(date);
    out
}

/// The JPEG with `DateTimeOriginal` set to `iso` (`Some`) or removed (`None`), or `None` when the
/// file is not a JPEG this writer understands (the caller then refuses the change).
pub fn set_date_taken(jpeg: &[u8], iso: Option<&str>) -> Option<Vec<u8>> {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let date = match iso {
        Some(text) => Some(exif_date_bytes(text)?),
        None => None,
    };
    let Some((segment, payload, end)) = find_exif(jpeg) else {
        // No EXIF: nothing to remove, or a new small block.
        let date = date?;
        let at = insert_point(jpeg);
        let mut out = jpeg[..at].to_vec();
        out.extend_from_slice(&app1(&new_tiff(&date))?);
        out.extend_from_slice(&jpeg[at..]);
        return Some(out);
    };
    let old = &jpeg[payload + 6..end];
    let little = match old.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Tiff { data: old, little };
    let ifd0_at = usize::try_from(t.u32(4)?).ok()?;
    let (ifd0, ifd0_next) = t.ifd(ifd0_at)?;
    let pointer = ifd0.iter().position(|e| t.tag(e) == EXIF_IFD_POINTER);
    let (exif_entries, exif_next) = match pointer {
        Some(i) => {
            let at = usize::try_from(t.u32(ifd0_at + 2 + i * 12 + 8)?).ok()?;
            t.ifd(at)?
        }
        None => (Vec::new(), 0),
    };
    let mut tiff = old.to_vec();
    if tiff.len() % 2 == 1 {
        tiff.push(0);
    }
    // The new Exif IFD at the end, then the date value right after it.
    let mut entries: Vec<[u8; 12]> = exif_entries
        .into_iter()
        .filter(|e| t.tag(e) != DATE_TIME_ORIGINAL)
        .collect();
    let new_exif_at = u32::try_from(tiff.len()).ok()?;
    let count_with_date = entries.len() + usize::from(date.is_some());
    let date_at = new_exif_at + u32::try_from(2 + count_with_date * 12 + 4).ok()?;
    if let Some(_date) = date {
        let mut entry = Vec::with_capacity(12);
        t.put_u16(&mut entry, DATE_TIME_ORIGINAL);
        t.put_u16(&mut entry, ASCII);
        t.put_u32(&mut entry, 20);
        t.put_u32(&mut entry, date_at);
        entries.push(entry.try_into().ok()?);
    }
    entries.sort_by_key(|e| t.tag(e));
    t.put_u16(&mut tiff, u16::try_from(entries.len()).ok()?);
    for e in &entries {
        tiff.extend_from_slice(e);
    }
    t.put_u32(&mut tiff, exif_next);
    if let Some(date) = date {
        tiff.extend_from_slice(&date);
    }
    match pointer {
        // Only the pointer changes in place.
        Some(i) => {
            let at = ifd0_at + 2 + i * 12 + 8;
            tiff[at..at + 4].copy_from_slice(&t.u32_bytes(new_exif_at));
        }
        // IFD0 copied to the end with the pointer added; the header points to the copy.
        None => {
            let new_ifd0_at = u32::try_from(tiff.len()).ok()?;
            let mut list = ifd0.clone();
            let mut entry = Vec::with_capacity(12);
            t.put_u16(&mut entry, EXIF_IFD_POINTER);
            t.put_u16(&mut entry, LONG);
            t.put_u32(&mut entry, 1);
            t.put_u32(&mut entry, new_exif_at);
            list.push(entry.try_into().ok()?);
            list.sort_by_key(|e| t.tag(e));
            t.put_u16(&mut tiff, u16::try_from(list.len()).ok()?);
            for e in &list {
                tiff.extend_from_slice(e);
            }
            t.put_u32(&mut tiff, ifd0_next);
            tiff[4..8].copy_from_slice(&t.u32_bytes(new_ifd0_at));
        }
    }
    let mut out = jpeg[..segment].to_vec();
    out.extend_from_slice(&app1(&tiff)?);
    out.extend_from_slice(&jpeg[end..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::read_exif;
    use crate::exif::tests::{jpeg_with_exif, photo_fields};
    use crate::vollmond::safety::picture_hash;
    use exif::{In, Reader, Tag};
    use std::path::Path;

    /// Every EXIF field as text, except the date taken and the Exif pointer (which may move).
    fn other_fields(bytes: &[u8]) -> Vec<String> {
        let exif = Reader::new()
            .read_from_container(&mut std::io::Cursor::new(bytes))
            .expect("exif");
        let mut list: Vec<String> = exif
            .fields()
            .filter(|f| f.tag != Tag::DateTimeOriginal && f.tag != Tag::ExifIFDPointer)
            .map(|f| format!("{:?} {} {}", f.ifd_num, f.tag, f.display_value()))
            .collect();
        list.sort();
        list
    }

    fn taken(bytes: &[u8]) -> Option<String> {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("x.jpg");
        std::fs::write(&path, bytes).expect("write");
        read_exif(&path).and_then(|e| e.taken)
    }

    /// A photo with full EXIF and some picture data, without a date taken.
    fn photo_without_date() -> Vec<u8> {
        let mut fields = photo_fields();
        fields.retain(|(tag, _)| *tag != Tag::DateTimeOriginal);
        let mut jpeg = jpeg_with_exif(&fields);
        jpeg.truncate(jpeg.len() - 2);
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        jpeg.extend((0..4096u32).map(|i| (i * 7 % 251) as u8));
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    #[test]
    fn adds_the_date_and_keeps_every_other_field_and_the_picture() {
        let before = photo_without_date();
        let after = set_date_taken(&before, Some("2023-05-12T14:30:00")).expect("write");
        assert_eq!(taken(&after).as_deref(), Some("2023-05-12T14:30:00"));
        assert_eq!(other_fields(&after), other_fields(&before));
        let p = Path::new("x.jpg");
        assert_eq!(
            picture_hash(&after, p).expect("a"),
            picture_hash(&before, p).expect("b")
        );
        // The old EXIF bytes are all still there, at the same offsets (append only).
        let (_, payload, end) = find_exif(&before).expect("exif");
        let old = &before[payload + 6..end];
        let (_, new_payload, _) = find_exif(&after).expect("exif");
        let new = &after[new_payload + 6..new_payload + 6 + old.len()];
        let differing = old.iter().zip(new).filter(|(a, b)| a != b).count();
        assert!(
            differing <= 4,
            "only the Exif pointer may change ({differing} bytes)"
        );
    }

    #[test]
    fn changes_and_removes_the_date() {
        let with = set_date_taken(&photo_without_date(), Some("2023-05-12T14:30:00")).expect("add");
        let changed = set_date_taken(&with, Some("2021-01-02T03:04:05")).expect("change");
        assert_eq!(taken(&changed).as_deref(), Some("2021-01-02T03:04:05"));
        let removed = set_date_taken(&changed, None).expect("remove");
        assert_eq!(taken(&removed), None);
        assert_eq!(other_fields(&removed), other_fields(&photo_without_date()));
    }

    #[test]
    fn a_jpeg_without_exif_gets_a_small_block_after_jfif() {
        let plain = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/source/f-1.jpg"),
        )
        .expect("jpeg");
        assert!(find_exif(&plain).is_none());
        let after = set_date_taken(&plain, Some("2023-05-12T12:00:00")).expect("write");
        assert_eq!(taken(&after).as_deref(), Some("2023-05-12T12:00:00"));
        assert_eq!(&after[2..4], &[0xFF, 0xE0], "JFIF stays first");
        let p = Path::new("x.jpg");
        assert_eq!(
            picture_hash(&after, p).expect("a"),
            picture_hash(&plain, p).expect("b")
        );
        let image = image::load_from_memory(&after).expect("still a JPEG");
        assert_eq!((image.width(), image.height()), (160, 120));
        assert!(set_date_taken(&plain, None).is_none(), "nothing to remove");
    }

    #[test]
    fn exif_without_an_exif_ifd_and_big_endian() {
        // orientation-*.jpg have IFD0 with only Orientation (little endian, no Exif IFD).
        let src = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/orientation-6.jpg"),
        )
        .expect("jpeg");
        let after = set_date_taken(&src, Some("2023-05-12T14:30:00")).expect("write");
        assert_eq!(taken(&after).as_deref(), Some("2023-05-12T14:30:00"));
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("o.jpg");
        std::fs::write(&path, &after).expect("write");
        assert_eq!(read_exif(&path).and_then(|e| e.orientation), Some(6));
        // Big endian EXIF.
        let mut be = b"MM\0*\0\0\0\x08\0\x01".to_vec();
        be.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 3, 0, 0, 0, 0, 0, 0]);
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&app1(&be).expect("app1"));
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0, 2, 1, 2, 3, 0xFF, 0xD9]);
        let after = set_date_taken(&jpeg, Some("2020-02-29T23:59:59")).expect("be");
        assert_eq!(taken(&after).as_deref(), Some("2020-02-29T23:59:59"));
        let exif = Reader::new()
            .read_from_container(&mut std::io::Cursor::new(&after))
            .expect("exif");
        assert_eq!(
            exif.get_field(Tag::Orientation, In::PRIMARY)
                .and_then(|f| f.value.get_uint(0)),
            Some(3)
        );
    }

    #[test]
    fn refuses_what_it_does_not_understand() {
        assert!(set_date_taken(b"\x89PNG", Some("2023-05-12T14:30:00")).is_none());
        assert!(set_date_taken(&photo_without_date(), Some("not a date")).is_none());
        let broken =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/broken.jpg"))
                .expect("broken");
        // A truncated header: either refused or written without touching what follows.
        let _ = set_date_taken(&broken, Some("2023-05-12T14:30:00"));
    }
}
