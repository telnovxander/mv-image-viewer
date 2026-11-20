//! Vollmond (IV-13…IV-19): the tools for a photo archive. Everything here follows
//! IV/PROJECT.md, "Безопасность (ОБЯЗАТЕЛЬНО)": preview before any change, journal with undo,
//! atomic writes, deleting only to the Recycle Bin (`crate::recycle`), tests only on `testdata/`.

pub mod dates;
pub mod dates_tool;
pub mod duplicates;
pub mod exif_write;
pub mod index;
pub mod manual_dates;
pub mod plan;
pub mod safety;
pub mod similar;
pub mod video;
