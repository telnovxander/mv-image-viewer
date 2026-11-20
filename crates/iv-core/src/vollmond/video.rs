//! Dates of videos (IV-16): MP4 and MOV keep their creation time in QuickTime boxes — `mvhd` of
//! the movie, `tkhd` and `mdhd` of each track — as seconds since 1904-01-01 UTC. Spiegelmond reads
//! and writes only these numbers; it never plays videos.
//!
//! Writing changes the value fields **in place** (4 or 8 bytes each, the same size), so the file
//! keeps its length and every byte of the media data (`mdat`) stays as it was — checked by hashing
//! the media data before and after (point 9 for videos).

use std::path::Path;

use crate::error::{ErrorCode, IvError};

/// Video extensions whose dates the tools handle.
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "3gp"];

/// Seconds between 1904-01-01 and 1970-01-01.
const MAC_EPOCH: i64 = 2_082_844_800;

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// A time field of a box: where it is and how wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field {
    at: usize,
    wide: bool,
}

/// Byte ranges of the media data.
type Ranges<T> = Vec<(T, T)>;

fn be32(data: &[u8], at: usize) -> Option<u64> {
    let b = data.get(at..at + 4)?;
    Some(u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
}

fn be64(data: &[u8], at: usize) -> Option<u64> {
    let b = data.get(at..at + 8)?;
    Some(u64::from_be_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// The children of a box payload `data[start..end]`: (kind, payload start, end).
fn boxes(data: &[u8], start: usize, end: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut at = start;
    while at + 8 <= end {
        let Some(size) = be32(data, at) else { break };
        let kind: [u8; 4] = match data[at + 4..at + 8].try_into() {
            Ok(k) => k,
            Err(_) => break,
        };
        let (payload, total) = match size {
            1 => match be64(data, at + 8) {
                Some(big) => (at + 16, usize::try_from(big).unwrap_or(usize::MAX)),
                None => break,
            },
            0 => (at + 8, end - at),
            n => (at + 8, usize::try_from(n).unwrap_or(usize::MAX)),
        };
        if total < 8 || at.checked_add(total).is_none_or(|e| e > end) {
            break;
        }
        out.push((kind, payload, at + total));
        at += total;
    }
    out
}

/// The creation-time fields: `mvhd` first, then every `tkhd` and `mdhd`; and the media data range.
fn locate(data: &[u8]) -> Option<(Vec<Field>, Ranges<usize>)> {
    let mut fields = Vec::new();
    let mut media = Vec::new();
    let top = boxes(data, 0, data.len());
    for (kind, payload, end) in &top {
        match kind {
            b"mdat" => media.push((*payload, *end)),
            b"moov" => {
                for (kind, payload, end) in boxes(data, *payload, *end) {
                    match &kind {
                        b"mvhd" => fields.push(Field {
                            at: payload + 4,
                            wide: data.get(payload) == Some(&1),
                        }),
                        b"trak" => {
                            for (kind, payload, end) in boxes(data, payload, end) {
                                match &kind {
                                    b"tkhd" => fields.push(Field {
                                        at: payload + 4,
                                        wide: data.get(payload) == Some(&1),
                                    }),
                                    b"mdia" => {
                                        for (kind, payload, _) in boxes(data, payload, end) {
                                            if &kind == b"mdhd" {
                                                fields.push(Field {
                                                    at: payload + 4,
                                                    wide: data.get(payload) == Some(&1),
                                                });
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    (!fields.is_empty()).then_some((fields, media))
}

fn read_field(data: &[u8], f: Field) -> Option<u64> {
    if f.wide {
        be64(data, f.at)
    } else {
        be32(data, f.at)
    }
}

/// The creation time of the movie (`mvhd`) in seconds since 1970 UTC; `None` without one (0 means
/// "not set" in QuickTime).
pub fn creation_time(data: &[u8]) -> Option<i64> {
    let (fields, _) = locate(data)?;
    let value = read_field(data, *fields.first()?)?;
    (value != 0).then(|| i64::try_from(value).ok().map(|v| v - MAC_EPOCH))?
}

/// A fingerprint of the media data (all `mdat` boxes): it must never change (point 9).
pub fn media_hash(data: &[u8]) -> Option<String> {
    let (_, media) = locate(data)?;
    let mut hasher = blake3::Hasher::new();
    for (start, end) in media {
        hasher.update(data.get(start..end)?);
    }
    Some(hasher.finalize().to_hex().to_string())
}

/// The video with every creation time (movie, tracks, media) set to `seconds` since 1970 UTC, or
/// cleared (`None`, QuickTime's 0). The length and everything else stay the same.
pub fn with_creation_time(
    data: &[u8],
    seconds: Option<i64>,
    path: &Path,
) -> Result<Vec<u8>, IvError> {
    let (fields, _) = locate(data).ok_or_else(|| IvError::new(ErrorCode::Unsupported, path))?;
    let value = match seconds {
        Some(s) => {
            u64::try_from(s + MAC_EPOCH).map_err(|_| IvError::new(ErrorCode::Other, path))?
        }
        None => 0,
    };
    let mut out = data.to_vec();
    for f in fields {
        if f.wide {
            out[f.at..f.at + 8].copy_from_slice(&value.to_be_bytes());
        } else {
            let narrow = u32::try_from(value).map_err(|_| IvError::new(ErrorCode::Other, path))?;
            out[f.at..f.at + 4].copy_from_slice(&narrow.to_be_bytes());
        }
    }
    Ok(out)
}

/// The time fields and media ranges of a video file, found by reading only box headers and the
/// `moov` box (a few hundred kilobytes even for hours of video).
fn scan_file(file: &mut std::fs::File) -> Option<(Vec<Field>, Ranges<u64>)> {
    use std::io::{Read, Seek, SeekFrom};
    let length = file.metadata().ok()?.len();
    let mut at = 0u64;
    let mut fields = Vec::new();
    let mut media = Vec::new();
    while at + 8 <= length {
        file.seek(SeekFrom::Start(at)).ok()?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header[..8]).ok()?;
        let size = u64::from(u32::from_be_bytes([
            header[0], header[1], header[2], header[3],
        ]));
        let (payload, total) = match size {
            1 => {
                file.read_exact(&mut header[8..16]).ok()?;
                let big = u64::from_be_bytes(header[8..16].try_into().ok()?);
                (at + 16, big)
            }
            0 => (at + 8, length - at),
            n => (at + 8, n),
        };
        if total < 8 || at + total > length {
            return None;
        }
        match &header[4..8] {
            b"mdat" => media.push((payload, at + total)),
            b"moov" => {
                // Read the whole moov box and find the fields in it; offsets back to the file.
                let mut moov = vec![0u8; usize::try_from(total).ok()?];
                file.seek(SeekFrom::Start(at)).ok()?;
                file.read_exact(&mut moov).ok()?;
                let (found, _) = locate(&moov)?;
                let base = usize::try_from(at).ok()?;
                fields.extend(found.into_iter().map(|f| Field {
                    at: f.at + base,
                    wide: f.wide,
                }));
            }
            _ => {}
        }
        at += total;
    }
    (!fields.is_empty()).then_some((fields, media))
}

fn read_at(file: &mut std::fs::File, f: Field) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(u64::try_from(f.at).ok()?)).ok()?;
    let mut b = [0u8; 8];
    if f.wide {
        file.read_exact(&mut b).ok()?;
        Some(u64::from_be_bytes(b))
    } else {
        file.read_exact(&mut b[..4]).ok()?;
        Some(u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
    }
}

/// The creation time of a video file, seconds since 1970 UTC.
pub fn file_creation_time(path: &Path) -> Option<i64> {
    let mut file = std::fs::File::open(path).ok()?;
    let (fields, _) = scan_file(&mut file)?;
    let value = read_at(&mut file, *fields.first()?)?;
    (value != 0).then(|| i64::try_from(value).ok().map(|v| v - MAC_EPOCH))?
}

/// BLAKE3 of the media data of a file, read in blocks.
fn file_media_hash(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let (_, media) = scan_file(&mut file)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    for (start, end) in media {
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut left = end - start;
        while left > 0 {
            let n = usize::try_from(left.min(buffer.len() as u64)).ok()?;
            file.read_exact(&mut buffer[..n]).ok()?;
            hasher.update(&buffer[..n]);
            left -= n as u64;
        }
    }
    Some(hasher.finalize().to_hex().to_string())
}

/// Set every creation time of the video at `path` (points 8–10 for videos): the file is copied next
/// to itself, the fields are patched in the copy, the copy is checked (time read back, media data
/// identical) and renamed over the original; its file date is kept.
pub fn set_file_creation_time(path: &Path, seconds: Option<i64>) -> Result<(), IvError> {
    use std::io::{Seek, SeekFrom, Write};
    let failed = |code| IvError::new(code, path);
    let kept = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(|e| IvError::from_io(&e, path))?;
    let before = file_media_hash(path).ok_or_else(|| failed(ErrorCode::Unsupported))?;
    let value = match seconds {
        Some(s) => u64::try_from(s + MAC_EPOCH).map_err(|_| failed(ErrorCode::Other))?,
        None => 0,
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{name}.iv-tmp"));
    let result = (|| {
        std::fs::copy(path, &temp).map_err(|e| IvError::from_io(&e, &temp))?;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temp)
            .map_err(|e| IvError::from_io(&e, &temp))?;
        let (fields, _) = scan_file(&mut file).ok_or_else(|| failed(ErrorCode::Unsupported))?;
        for f in &fields {
            file.seek(SeekFrom::Start(f.at as u64))
                .map_err(|e| IvError::from_io(&e, &temp))?;
            let written = if f.wide {
                file.write_all(&value.to_be_bytes())
            } else {
                let narrow = u32::try_from(value).map_err(|_| failed(ErrorCode::Other))?;
                file.write_all(&narrow.to_be_bytes())
            };
            written.map_err(|e| IvError::from_io(&e, &temp))?;
        }
        file.sync_all().map_err(|e| IvError::from_io(&e, &temp))?;
        drop(file);
        if file_media_hash(&temp).as_deref() != Some(before.as_str()) {
            return Err(failed(ErrorCode::PictureChanged));
        }
        if file_creation_time(&temp) != seconds {
            return Err(failed(ErrorCode::VerifyFailed));
        }
        std::fs::rename(&temp, path).map_err(|e| IvError::from_io(&e, path))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return result;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|f| f.set_modified(kept))
        .map_err(|e| IvError::from_io(&e, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    #[test]
    fn reads_and_writes_the_dates_of_mp4_and_mov() {
        for name in ["sample.mp4", "sample.mov"] {
            let data = std::fs::read(testdata(name)).expect(name);
            // 2023-05-12T14:30:00Z, as make-testdata.py writes it.
            assert_eq!(creation_time(&data), Some(1_683_901_800), "{name}");
            let hash = media_hash(&data).expect("media");
            let changed =
                with_creation_time(&data, Some(1_000_000_000), Path::new(name)).expect("write");
            assert_eq!(changed.len(), data.len());
            assert_eq!(creation_time(&changed), Some(1_000_000_000));
            assert_eq!(
                media_hash(&changed).as_deref(),
                Some(hash.as_str()),
                "media data untouched"
            );
            let differing = data.iter().zip(&changed).filter(|(a, b)| a != b).count();
            assert!(differing <= 3 * 8, "only the time fields: {differing}");
            let cleared = with_creation_time(&changed, None, Path::new(name)).expect("clear");
            assert_eq!(creation_time(&cleared), None);
        }
    }

    #[test]
    fn file_functions_change_only_the_dates() {
        let temp = tempfile::tempdir().expect("temp");
        for name in ["sample.mp4", "sample.mov"] {
            let path = temp.path().join(name);
            std::fs::copy(testdata(name), &path).expect("copy");
            let before = std::fs::read(&path).expect("read");
            assert_eq!(file_creation_time(&path), Some(1_683_901_800));
            set_file_creation_time(&path, Some(1_000_000_000)).expect("write");
            assert_eq!(file_creation_time(&path), Some(1_000_000_000));
            let after = std::fs::read(&path).expect("read");
            assert_eq!(media_hash(&after), media_hash(&before));
            set_file_creation_time(&path, Some(1_683_901_800)).expect("back");
            assert_eq!(
                std::fs::read(&path).expect("read"),
                before,
                "undo gives the same bytes"
            );
        }
        let broken = temp.path().join("broken.mp4");
        std::fs::copy(testdata("broken.mp4"), &broken).expect("copy");
        assert!(set_file_creation_time(&broken, Some(0)).is_err());
        assert_eq!(
            std::fs::read_dir(temp.path()).expect("dir").count(),
            3,
            "no temporary files left"
        );
    }

    #[test]
    fn broken_and_foreign_files_are_refused() {
        let broken = std::fs::read(testdata("broken.mp4")).expect("broken");
        assert_eq!(creation_time(&broken), None);
        assert!(with_creation_time(&broken, Some(0), Path::new("x")).is_err());
        assert!(with_creation_time(b"not a video", Some(0), Path::new("x")).is_err());
        assert!(is_video(Path::new("VID_20230512_143000.MP4")));
        assert!(!is_video(Path::new("a.jpg")));
    }
}
