//! Vollmond (IV-13…IV-19): the tools for a photo archive. Everything here follows
//! IV/PROJECT.md, "Безопасность (ОБЯЗАТЕЛЬНО)": preview before any change, journal with undo,
//! atomic writes, deleting only to the Recycle Bin (`crate::recycle`), tests only on `testdata/`.

pub mod index;
