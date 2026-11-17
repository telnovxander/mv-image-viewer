//! EXIF of a photo (IV-04): orientation, date taken, camera, lens, exposure, GPS. Reading only;
//! a file is never changed here.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Reader, Tag, Value};
use serde::Serialize;

/// What the information panel and the sorting need. Missing fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ExifInfo {
    /// EXIF Orientation 1–8 (1 = as stored); `None` without the tag.
    pub orientation: Option<u16>,
    /// Date taken as written by the camera, `YYYY-MM-DDTHH:MM:SS` (no time zone).
    pub taken: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    /// Exposure time in seconds.
    pub exposure: Option<f64>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    /// Focal length in millimetres.
    pub focal_length: Option<f64>,
    /// Latitude and longitude in degrees (south and west negative).
    pub gps: Option<(f64, f64)>,
    /// Pixel size as the camera recorded it.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// The EXIF of `path`, or `None` if the file has none or cannot be read: a photo without EXIF
/// is normal, not an error.
pub fn read_exif(path: &Path) -> Option<ExifInfo> {
    let file = File::open(path).ok()?;
    let exif = Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;
    let field = |tag: Tag| exif.get_field(tag, In::PRIMARY);
    let uint = |tag: Tag| field(tag).and_then(|f| f.value.get_uint(0));
    let ascii = |tag: Tag| {
        field(tag).and_then(|f| match &f.value {
            Value::Ascii(parts) => parts
                .first()
                .map(|bytes| String::from_utf8_lossy(bytes).trim().to_string())
                .filter(|s| !s.is_empty()),
            _ => None,
        })
    };
    let rational = |tag: Tag| {
        field(tag).and_then(|f| match &f.value {
            Value::Rational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
            _ => None,
        })
    };

    let make = ascii(Tag::Make);
    let model = ascii(Tag::Model);
    let camera = match (make, model) {
        // Many cameras repeat the maker in the model: "Canon" + "Canon EOS R6" → "Canon EOS R6".
        (Some(make), Some(model)) if model.to_lowercase().starts_with(&make.to_lowercase()) => {
            Some(model)
        }
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (make, model) => make.or(model),
    };

    Some(ExifInfo {
        orientation: uint(Tag::Orientation)
            .and_then(|o| u16::try_from(o).ok())
            .filter(|o| (1..=8).contains(o)),
        taken: ascii(Tag::DateTimeOriginal)
            .or_else(|| ascii(Tag::DateTime))
            .and_then(|text| exif_date(&text)),
        camera,
        lens: ascii(Tag::LensModel),
        exposure: rational(Tag::ExposureTime),
        f_number: rational(Tag::FNumber),
        iso: uint(Tag::PhotographicSensitivity),
        focal_length: rational(Tag::FocalLength),
        gps: gps(&exif),
        width: uint(Tag::PixelXDimension),
        height: uint(Tag::PixelYDimension),
    })
}

/// "2023:05:12 14:30:00" → "2023-05-12T14:30:00"; anything else (or "0000:00:00") → None.
pub fn exif_date(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.len() < 19 || bytes[4] != b':' || bytes[7] != b':' || bytes[10] != b' ' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<u32>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if year == 0
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}"
    ))
}

/// The OpenStreetMap page of a place (opened in the browser only on a click).
pub fn map_url(latitude: f64, longitude: f64) -> String {
    format!("https://www.openstreetmap.org/?mlat={latitude:.6}&mlon={longitude:.6}#map=16/{latitude:.6}/{longitude:.6}")
}

fn gps(exif: &exif::Exif) -> Option<(f64, f64)> {
    let degrees = |tag: Tag, reference: Tag, negative: u8| {
        let value = match &exif.get_field(tag, In::PRIMARY)?.value {
            Value::Rational(v) if v.len() >= 3 && v.iter().all(|r| r.denom != 0) => {
                v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0
            }
            _ => return None,
        };
        let sign = match &exif.get_field(reference, In::PRIMARY)?.value {
            Value::Ascii(parts) if parts.first().and_then(|p| p.first()) == Some(&negative) => -1.0,
            _ => 1.0,
        };
        Some(sign * value)
    };
    Some((
        degrees(Tag::GPSLatitude, Tag::GPSLatitudeRef, b'S')?,
        degrees(Tag::GPSLongitude, Tag::GPSLongitudeRef, b'W')?,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use exif::experimental::Writer;
    use exif::{Field, Rational};
    use std::io::Cursor;

    fn ascii(text: &str) -> Value {
        Value::Ascii(vec![text.as_bytes().to_vec()])
    }

    fn rational(parts: &[(u32, u32)]) -> Value {
        Value::Rational(
            parts
                .iter()
                .map(|&(num, denom)| Rational { num, denom })
                .collect(),
        )
    }

    /// A JPEG holding only an EXIF segment with the given fields (enough for the reader).
    pub(crate) fn jpeg_with_exif(fields: &[(Tag, Value)]) -> Vec<u8> {
        let fields: Vec<Field> = fields
            .iter()
            .map(|(tag, value)| Field {
                tag: *tag,
                ifd_num: In::PRIMARY,
                value: value.clone(),
            })
            .collect();
        let mut writer = Writer::new();
        for field in &fields {
            writer.push_field(field);
        }
        let mut tiff = Cursor::new(Vec::new());
        writer.write(&mut tiff, false).expect("write exif");
        let tiff = tiff.into_inner();
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let length = u16::try_from(tiff.len() + 8).expect("small");
        jpeg.extend_from_slice(&length.to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    pub(crate) fn photo_fields() -> Vec<(Tag, Value)> {
        vec![
            (Tag::Orientation, Value::Short(vec![6])),
            (Tag::Make, ascii("Canon")),
            (Tag::Model, ascii("Canon EOS R6")),
            (Tag::DateTimeOriginal, ascii("2023:05:12 14:30:00")),
            (Tag::LensModel, ascii("RF24-105mm F4 L IS USM")),
            (Tag::ExposureTime, rational(&[(1, 250)])),
            (Tag::FNumber, rational(&[(56, 10)])),
            (Tag::PhotographicSensitivity, Value::Short(vec![400])),
            (Tag::FocalLength, rational(&[(50, 1)])),
            (Tag::GPSLatitudeRef, ascii("N")),
            (Tag::GPSLatitude, rational(&[(47, 1), (22, 1), (3000, 100)])),
            (Tag::GPSLongitudeRef, ascii("E")),
            (Tag::GPSLongitude, rational(&[(8, 1), (32, 1), (0, 1)])),
            (Tag::PixelXDimension, Value::Long(vec![6000])),
            (Tag::PixelYDimension, Value::Long(vec![4000])),
        ]
    }

    #[test]
    fn reads_every_field_of_a_photo() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("photo.jpg");
        std::fs::write(&path, jpeg_with_exif(&photo_fields())).expect("write");
        let info = read_exif(&path).expect("exif");
        assert_eq!(info.orientation, Some(6));
        assert_eq!(info.taken.as_deref(), Some("2023-05-12T14:30:00"));
        assert_eq!(info.camera.as_deref(), Some("Canon EOS R6"));
        assert_eq!(info.lens.as_deref(), Some("RF24-105mm F4 L IS USM"));
        assert_eq!(info.exposure, Some(1.0 / 250.0));
        assert_eq!(info.f_number, Some(5.6));
        assert_eq!(info.iso, Some(400));
        assert_eq!(info.focal_length, Some(50.0));
        let (lat, lon) = info.gps.expect("gps");
        assert!((lat - 47.375).abs() < 1e-9 && (lon - 8.533_333).abs() < 1e-5);
        assert_eq!((info.width, info.height), (Some(6000), Some(4000)));
    }

    #[test]
    fn all_eight_orientations() {
        let temp = tempfile::tempdir().expect("temp dir");
        for orientation in 1..=8u16 {
            let path = temp.path().join(format!("o{orientation}.jpg"));
            let jpeg = jpeg_with_exif(&[(Tag::Orientation, Value::Short(vec![orientation]))]);
            std::fs::write(&path, jpeg).expect("write");
            assert_eq!(
                read_exif(&path).expect("exif").orientation,
                Some(orientation)
            );
        }
    }

    #[test]
    fn testdata_has_all_eight_orientations() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
        for code in 1..=8u16 {
            let path = data.join(format!("orientation-{code}.jpg"));
            assert_eq!(
                read_exif(&path).and_then(|e| e.orientation),
                Some(code),
                "{code}"
            );
        }
    }

    #[test]
    fn south_west_and_maker_not_repeated() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("sw.jpg");
        let jpeg = jpeg_with_exif(&[
            (Tag::Make, ascii("FUJIFILM")),
            (Tag::Model, ascii("X-T4")),
            (Tag::GPSLatitudeRef, ascii("S")),
            (Tag::GPSLatitude, rational(&[(33, 1), (52, 1), (0, 1)])),
            (Tag::GPSLongitudeRef, ascii("W")),
            (Tag::GPSLongitude, rational(&[(70, 1), (30, 1), (0, 1)])),
        ]);
        std::fs::write(&path, jpeg).expect("write");
        let info = read_exif(&path).expect("exif");
        assert_eq!(info.camera.as_deref(), Some("FUJIFILM X-T4"));
        let (lat, lon) = info.gps.expect("gps");
        assert!(lat < 0.0 && lon < 0.0);
    }

    #[test]
    fn no_exif_is_none_not_an_error() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
        assert_eq!(read_exif(&data.join("red.png")), None);
        assert_eq!(read_exif(&data.join("broken.jpg")), None);
        assert_eq!(read_exif(&data.join("missing.jpg")), None);
    }

    #[test]
    fn map_link() {
        assert_eq!(
            map_url(47.375, -8.5),
            "https://www.openstreetmap.org/?mlat=47.375000&mlon=-8.500000#map=16/47.375000/-8.500000"
        );
    }

    #[test]
    fn exif_dates() {
        assert_eq!(
            exif_date("2023:05:12 14:30:00").as_deref(),
            Some("2023-05-12T14:30:00")
        );
        assert_eq!(exif_date("0000:00:00 00:00:00"), None);
        assert_eq!(exif_date("2023-05-12"), None);
        assert_eq!(exif_date("2023:13:12 14:30:00"), None);
    }
}
