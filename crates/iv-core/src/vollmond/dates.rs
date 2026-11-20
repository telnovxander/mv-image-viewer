//! Dates of photos (IV-14): the date taken from EXIF, the date in the file name (phones and
//! messengers write it there), the file's modification time — and which one is right.
//!
//! Phone galleries sort by the modification time, which copying resets to "today": the main
//! trouble this tool fixes. The rule: EXIF date taken → date in the name → leave as it is. A file
//! without an EXIF date gets the one from its name written into EXIF (`DateTimeOriginal`); the
//! modification time follows the date taken. Suspicious dates (in the future, before 1990) are
//! marked and not applied unless chosen (point 13); an existing EXIF date is never overwritten
//! automatically (point 12).

use serde::Serialize;

use super::plan::{Change, Plan};

/// A date and time without a zone, as cameras write it: `YYYY-MM-DDTHH:MM:SS` (also its form for
/// the window: a short string instead of six fields keeps a table of 50 000 rows small).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Stamp {
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
            2 => 28,
            _ => return None,
        };
        let valid = (1900..=2200).contains(&year)
            && (1..=days).contains(&day)
            && hour < 24
            && minute < 60
            && second < 60;
        valid.then_some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// `2023-05-12T14:30:00` (also accepts the EXIF form `2023:05:12 14:30:00`).
    pub fn parse(text: &str) -> Option<Self> {
        let digits: Vec<u32> = text
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse().ok())
            .collect::<Option<Vec<_>>>()?;
        let [y, mo, d, h, mi, s, ..] = digits[..] else {
            return None;
        };
        Self::new(
            u16::try_from(y).ok()?,
            u8::try_from(mo).ok()?,
            u8::try_from(d).ok()?,
            u8::try_from(h).ok()?,
            u8::try_from(mi).ok()?,
            u8::try_from(s).ok()?,
        )
    }

    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// Seconds since 1970 as if the stamp were UTC (only differences are compared).
    pub fn seconds(&self) -> i64 {
        let (y, m) = if self.month <= 2 {
            (i64::from(self.year) - 1, i64::from(self.month) + 9)
        } else {
            (i64::from(self.year), i64::from(self.month) - 3)
        };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }

    /// The local wall-clock time of `seconds` since 1970 with the zone offset `offset_seconds`.
    pub fn from_seconds(seconds: i64, offset_seconds: i64) -> Option<Self> {
        let t = seconds + offset_seconds;
        let days = t.div_euclid(86_400);
        let rest = t.rem_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self::new(
            u16::try_from(year).ok()?,
            u8::try_from(month).ok()?,
            u8::try_from(day).ok()?,
            u8::try_from(rest / 3600).ok()?,
            u8::try_from(rest % 3600 / 60).ok()?,
            u8::try_from(rest % 60).ok()?,
        )
    }
}

impl Serialize for Stamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.iso())
    }
}

/// Which naming scheme a date was read from (shown in the table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NameScheme {
    /// `IMG_20230512_143000`, `VID_…`, `PXL_20230512_143000123`, `20230512_143000` (Xiaomi,
    /// Samsung, Pixel and most Android cameras).
    Camera,
    /// `IMG-20230512-WA0001` (WhatsApp): the day only.
    WhatsApp,
    /// `photo_2023-05-12_14-30-00` (Telegram desktop).
    Telegram,
    /// `Screenshot_2023-05-12-14-30-00-123_…`, `Screenshot_20230512_143000`, `Screenshot 2023-05-12 143000`.
    Screenshot,
    /// `2023-05-12 14.30.00` (Dropbox camera upload), `2023-05-12_14-30-00`.
    Dated,
    /// Only a day: `2023-05-12`, `20230512` somewhere in the name.
    Day,
}

/// A date read from a file name, and how sure: a day-only scheme gives noon of that day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NameDate {
    pub stamp: Stamp,
    pub scheme: NameScheme,
    /// The name gives the day only (the time is 12:00:00).
    pub day_only: bool,
}

fn number(text: &str) -> Option<u32> {
    text.parse().ok()
}

/// Groups of digits of a name, with the position where each starts.
fn digit_runs(name: &str) -> Vec<(usize, &str)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, c) in name.char_indices() {
        match (c.is_ascii_digit(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                runs.push((s, &name[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push((s, &name[s..]));
    }
    runs
}

fn ymd(text: &str) -> Option<(u16, u8, u8)> {
    (text.len() == 8).then_some(())?;
    Some((
        u16::try_from(number(&text[0..4])?).ok()?,
        u8::try_from(number(&text[4..6])?).ok()?,
        u8::try_from(number(&text[6..8])?).ok()?,
    ))
}

fn hms(text: &str) -> Option<(u8, u8, u8)> {
    (text.len() >= 6).then_some(())?;
    Some((
        u8::try_from(number(&text[0..2])?).ok()?,
        u8::try_from(number(&text[2..4])?).ok()?,
        u8::try_from(number(&text[4..6])?).ok()?,
    ))
}

/// The date in a file name, if it follows one of the known schemes.
pub fn date_from_name(file_name: &str) -> Option<NameDate> {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let lower = stem.to_ascii_lowercase();
    let runs = digit_runs(stem);
    let digits: Vec<&str> = runs.iter().map(|r| r.1).collect();
    let full = |scheme, y: (u16, u8, u8), t: (u8, u8, u8)| {
        Stamp::new(y.0, y.1, y.2, t.0, t.1, t.2).map(|stamp| NameDate {
            stamp,
            scheme,
            day_only: false,
        })
    };
    let day = |scheme, y: (u16, u8, u8)| {
        Stamp::new(y.0, y.1, y.2, 12, 0, 0).map(|stamp| NameDate {
            stamp,
            scheme,
            day_only: true,
        })
    };
    let screenshot = lower.starts_with("screenshot");
    // WhatsApp: IMG-20230512-WA0001 (day only).
    if lower.contains("-wa") {
        if let Some(date) = digits.first().and_then(|d| ymd(d)) {
            return day(NameScheme::WhatsApp, date);
        }
    }
    // YYYYMMDD then HHMMSS[mmm]: camera apps, compact screenshots.
    for pair in digits.windows(2) {
        if let (Some(date), Some(time)) = (ymd(pair[0]), hms(pair[1])) {
            if pair[1].len() <= 9 {
                let scheme = if screenshot {
                    NameScheme::Screenshot
                } else {
                    NameScheme::Camera
                };
                if let Some(found) = full(scheme, date, time) {
                    return Some(found);
                }
            }
        }
    }
    // YYYY MM DD HH MM SS as six groups: Telegram, screenshots, Dropbox and similar.
    for six in digits.windows(6) {
        let parts: Option<Vec<u32>> = six.iter().map(|p| number(p)).collect();
        let Some(p) = parts else { continue };
        if six[0].len() == 4 && six[1..].iter().all(|x| x.len() == 2) {
            let scheme = if lower.starts_with("photo_") || lower.starts_with("video_") {
                NameScheme::Telegram
            } else if screenshot {
                NameScheme::Screenshot
            } else {
                NameScheme::Dated
            };
            let (Ok(y), Ok(mo), Ok(d), Ok(h), Ok(mi), Ok(s)) = (
                u16::try_from(p[0]),
                u8::try_from(p[1]),
                u8::try_from(p[2]),
                u8::try_from(p[3]),
                u8::try_from(p[4]),
                u8::try_from(p[5]),
            ) else {
                continue;
            };
            if let Some(found) = full(scheme, (y, mo, d), (h, mi, s)) {
                return Some(found);
            }
        }
    }
    // YYYY MM DD then HHMMSS: "Screenshot 2023-05-12 143000".
    for four in digits.windows(4) {
        if four[0].len() == 4 && four[1].len() == 2 && four[2].len() == 2 && four[3].len() == 6 {
            let (Some(y), Some(mo), Some(d)) = (number(four[0]), number(four[1]), number(four[2]))
            else {
                continue;
            };
            let date = (
                u16::try_from(y).ok()?,
                u8::try_from(mo).ok()?,
                u8::try_from(d).ok()?,
            );
            if let Some(time) = hms(four[3]) {
                let scheme = if screenshot {
                    NameScheme::Screenshot
                } else {
                    NameScheme::Dated
                };
                if let Some(found) = full(scheme, date, time) {
                    return Some(found);
                }
            }
        }
    }
    // A day alone: YYYY-MM-DD or YYYYMMDD.
    for three in digits.windows(3) {
        if three[0].len() == 4 && three[1].len() == 2 && three[2].len() == 2 {
            let (Some(y), Some(mo), Some(d)) =
                (number(three[0]), number(three[1]), number(three[2]))
            else {
                continue;
            };
            if let (Ok(y), Ok(mo), Ok(d)) = (u16::try_from(y), u8::try_from(mo), u8::try_from(d)) {
                if let Some(found) = day(NameScheme::Day, (y, mo, d)) {
                    return Some(found);
                }
            }
        }
    }
    digits
        .iter()
        .find_map(|d| ymd(d).and_then(|date| day(NameScheme::Day, date)))
}

/// One row of the dates table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DateRow {
    pub path: String,
    pub taken: Option<Stamp>,
    pub from_name: Option<NameDate>,
    /// The file's modification time, local wall clock.
    pub modified: Option<Stamp>,
    /// What the file should say, by the rule EXIF → name → nothing.
    pub proposed: Option<Stamp>,
    /// EXIF and the name disagree by more than a day, or the modification time is off.
    pub mismatch: bool,
    /// The proposed date is in the future or before 1990 (point 13).
    pub suspicious: bool,
}

/// How far the modification time may be from the date taken before it counts as wrong: cameras
/// write the file a moment after taking; time zones move it by hours.
pub const MODIFIED_SLACK_SECONDS: i64 = 26 * 3600;

/// Build the row of one file. `today` is the current local date, for "in the future".
pub fn row(path: &str, taken: Option<Stamp>, modified: Option<Stamp>, today: Stamp) -> DateRow {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let from_name = date_from_name(name);
    let proposed = taken.or(from_name.map(|n| n.stamp));
    let far = |a: Stamp, b: Stamp, slack: i64| (a.seconds() - b.seconds()).abs() > slack;
    let name_vs_exif = match (taken, from_name) {
        (Some(t), Some(n)) => far(t, n.stamp, if n.day_only { 36 * 3600 } else { 3 * 3600 }),
        _ => false,
    };
    let modified_off = match (proposed, modified) {
        (Some(p), Some(m)) => far(p, m, MODIFIED_SLACK_SECONDS),
        _ => false,
    };
    let suspicious = proposed.is_some_and(|p| p.year < 1990 || p > today);
    DateRow {
        path: path.to_owned(),
        taken,
        from_name,
        modified,
        proposed,
        mismatch: name_vs_exif || modified_off,
        suspicious,
    }
}

/// The table filter of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    All,
    Mismatches,
    NoDateTaken,
}

pub fn filter(rows: &[DateRow], which: Filter) -> Vec<&DateRow> {
    rows.iter()
        .filter(|r| match which {
            Filter::All => true,
            Filter::Mismatches => r.mismatch,
            Filter::NoDateTaken => r.taken.is_none(),
        })
        .collect()
}

/// The fixing plan for `rows`: EXIF date written only where there is none (from the name; point
/// 12), the modification time set to the date taken where it is off. Suspicious rows are kept in
/// the plan marked, and left out of applying unless the user chose them (point 13).
pub fn plan(rows: &[DateRow]) -> Plan {
    let mut changes = Vec::new();
    for r in rows {
        let Some(proposed) = r.proposed else { continue };
        if r.taken.is_none() {
            changes.push(Change {
                path: r.path.clone(),
                field: "taken".into(),
                before: None,
                after: Some(proposed.iso()),
                suspicious: r.suspicious,
            });
        }
        let modified_off = r
            .modified
            .is_none_or(|m| (m.seconds() - proposed.seconds()).abs() > MODIFIED_SLACK_SECONDS);
        if modified_off {
            changes.push(Change {
                path: r.path.clone(),
                field: "file_modified".into(),
                before: r.modified.map(|m| m.iso()),
                after: Some(proposed.iso()),
                suspicious: r.suspicious,
            });
        }
    }
    Plan {
        tool: "dates".into(),
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Stamp {
        Stamp::parse(text).expect(text)
    }

    #[test]
    fn names_of_phones_and_messengers() {
        let cases = [
            (
                "IMG_20230512_143000.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Camera,
                false,
            ),
            (
                "VID_20230512_143000.mp4",
                "2023-05-12T14:30:00",
                NameScheme::Camera,
                false,
            ),
            (
                "PXL_20230512_143000123.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Camera,
                false,
            ),
            (
                "20230512_143000.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Camera,
                false,
            ),
            (
                "IMG_20230512_143000_HDR.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Camera,
                false,
            ),
            (
                "IMG-20230512-WA0001.jpg",
                "2023-05-12T12:00:00",
                NameScheme::WhatsApp,
                true,
            ),
            (
                "VID-20230512-WA0003.mp4",
                "2023-05-12T12:00:00",
                NameScheme::WhatsApp,
                true,
            ),
            (
                "photo_2023-05-12_14-30-00.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Telegram,
                false,
            ),
            (
                "Screenshot_2023-05-12-14-30-00-123_com.whatsapp.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Screenshot,
                false,
            ),
            (
                "Screenshot_20230512_143000.png",
                "2023-05-12T14:30:00",
                NameScheme::Screenshot,
                false,
            ),
            (
                "Screenshot 2023-05-12 143000.png",
                "2023-05-12T14:30:00",
                NameScheme::Screenshot,
                false,
            ),
            (
                "2023-05-12 14.30.00.jpg",
                "2023-05-12T14:30:00",
                NameScheme::Dated,
                false,
            ),
            (
                "Urlaub 2023-05-12.jpg",
                "2023-05-12T12:00:00",
                NameScheme::Day,
                true,
            ),
            (
                "scan_19870704.tif",
                "1987-07-04T12:00:00",
                NameScheme::Day,
                true,
            ),
        ];
        for (name, expected, scheme, day_only) in cases {
            let found = date_from_name(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(found.stamp.iso(), expected, "{name}");
            assert_eq!((found.scheme, found.day_only), (scheme, day_only), "{name}");
        }
        for nothing in [
            "DSC_0001.jpg",
            "IMG_1234.HEIC",
            "photo.jpg",
            "IMG_20231399_250000.jpg",
            "99999999.jpg",
        ] {
            assert_eq!(date_from_name(nothing), None, "{nothing}");
        }
    }

    #[test]
    fn stamps_parse_compare_and_convert() {
        assert_eq!(s("2023:05:12 14:30:00"), s("2023-05-12T14:30:00"));
        assert!(Stamp::parse("2023-02-29T10:00:00").is_none());
        assert!(Stamp::parse("2024-02-29T10:00:00").is_some());
        assert_eq!(s("1970-01-01T00:00:00").seconds(), 0);
        assert_eq!(s("2000-03-01T00:00:01").seconds(), 951_868_801);
        let back = Stamp::from_seconds(951_868_801, 3600).expect("back");
        assert_eq!(back.iso(), "2000-03-01T01:00:01");
        assert_eq!(
            Stamp::from_seconds(s("2024-12-31T23:59:59").seconds(), 0),
            Some(s("2024-12-31T23:59:59"))
        );
    }

    #[test]
    fn mismatches_and_suspicious_dates() {
        let today = s("2026-11-20T12:00:00");
        // EXIF and name agree, the copy reset the modification time: a mismatch.
        let copied = row(
            "D:\\Fotos\\IMG_20230512_143000.jpg",
            Some(s("2023-05-12T14:30:00")),
            Some(s("2026-11-19T09:00:00")),
            today,
        );
        assert!(copied.mismatch && !copied.suspicious);
        assert_eq!(copied.proposed, Some(s("2023-05-12T14:30:00")));
        // All three agree (the file was written a second later): fine.
        let fine = row(
            "IMG_20230512_143000.jpg",
            Some(s("2023-05-12T14:30:00")),
            Some(s("2023-05-12T14:30:01")),
            today,
        );
        assert!(!fine.mismatch);
        // WhatsApp: no EXIF, the day from the name.
        let whatsapp = row(
            "IMG-20230512-WA0001.jpg",
            None,
            Some(s("2026-11-19T09:00:00")),
            today,
        );
        assert_eq!(whatsapp.proposed, Some(s("2023-05-12T12:00:00")));
        // EXIF says another year than the name: a mismatch, EXIF wins.
        let disagree = row(
            "IMG_20230512_143000.jpg",
            Some(s("2021-01-01T10:00:00")),
            None,
            today,
        );
        assert!(disagree.mismatch);
        assert_eq!(disagree.proposed, Some(s("2021-01-01T10:00:00")));
        // Point 13: in the future, or before 1990.
        assert!(row("IMG_20290101_100000.jpg", None, None, today).suspicious);
        assert!(row("scan_19870704.tif", None, None, today).suspicious);
        // No date anywhere: nothing proposed.
        assert_eq!(row("DSC_0001.jpg", None, Some(today), today).proposed, None);
    }

    #[test]
    fn filters() {
        let today = s("2026-11-20T12:00:00");
        let rows = vec![
            row(
                "IMG_20230512_143000.jpg",
                Some(s("2023-05-12T14:30:00")),
                Some(s("2023-05-12T14:30:00")),
                today,
            ),
            row("IMG-20230512-WA0001.jpg", None, Some(today), today),
        ];
        assert_eq!(filter(&rows, Filter::All).len(), 2);
        assert_eq!(filter(&rows, Filter::Mismatches).len(), 1);
        assert_eq!(filter(&rows, Filter::NoDateTaken).len(), 1);
    }

    #[test]
    fn the_plan_never_overwrites_an_exif_date() {
        let today = s("2026-11-20T12:00:00");
        let rows = vec![
            row(
                "IMG_20230512_143000.jpg",
                Some(s("2023-05-12T14:30:00")),
                Some(today),
                today,
            ),
            row("IMG-20230512-WA0001.jpg", None, Some(today), today),
            row("IMG_20290101_100000.jpg", None, None, today),
            row("DSC_0001.jpg", None, Some(today), today),
        ];
        let plan = plan(&rows);
        let fields: Vec<(&str, &str)> = plan
            .changes
            .iter()
            .map(|c| (c.path.as_str(), c.field.as_str()))
            .collect();
        assert_eq!(
            fields,
            [
                ("IMG_20230512_143000.jpg", "file_modified"),
                ("IMG-20230512-WA0001.jpg", "taken"),
                ("IMG-20230512-WA0001.jpg", "file_modified"),
                ("IMG_20290101_100000.jpg", "taken"),
                ("IMG_20290101_100000.jpg", "file_modified"),
            ]
        );
        assert!(plan
            .changes
            .iter()
            .filter(|c| c.path.starts_with("IMG_2029"))
            .all(|c| c.suspicious));
        assert!(plan
            .changes
            .iter()
            .all(|c| c.field != "taken" || c.before.is_none()));
    }
}
