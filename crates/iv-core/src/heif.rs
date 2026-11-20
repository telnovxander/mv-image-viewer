//! HEIC / HEIF (IV-05): decoded by libheif + libde265 (LGPL), loaded at run time as separate
//! libraries, never linked in (the LGPL allows that; their licences ship in `vendor/heif/`).
//! Windows: `heif.dll` (with `libde265.dll`) next to `iv.exe`, from `vendor/heif/` of the
//! repository; other systems: the system's `libheif.so.1` / `libheif.dylib` if there is one.
//! No crate: a few functions of the C API through `LoadLibraryW`/`dlopen`. Reading only.
//!
//! libheif applies the image's own transformations (rotation `irot`, mirror `imir`, crop `clap`)
//! while decoding, so the PNG it gives is already upright.

use std::ffi::{c_char, c_int, c_void};
use std::io::Cursor;
use std::sync::OnceLock;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

#[repr(C)]
struct HeifError {
    code: c_int,
    subcode: c_int,
    message: *const c_char,
}

type Ctx = c_void;
type Handle = c_void;
type Image = c_void;

const COLORSPACE_RGB: c_int = 1;
const CHROMA_INTERLEAVED_RGB: c_int = 10;
const CHROMA_INTERLEAVED_RGBA: c_int = 11;
const CHANNEL_INTERLEAVED: c_int = 10;

type ReadFromMemory =
    unsafe extern "C" fn(*mut Ctx, *const c_void, usize, *const c_void) -> HeifError;
type Decode =
    unsafe extern "C" fn(*const Handle, *mut *mut Image, c_int, c_int, *const c_void) -> HeifError;

/// The functions of libheif that are used.
struct Api {
    context_alloc: unsafe extern "C" fn() -> *mut Ctx,
    context_free: unsafe extern "C" fn(*mut Ctx),
    read_from_memory:
        unsafe extern "C" fn(*mut Ctx, *const c_void, usize, *const c_void) -> HeifError,
    primary_handle: unsafe extern "C" fn(*mut Ctx, *mut *mut Handle) -> HeifError,
    handle_release: unsafe extern "C" fn(*const Handle),
    has_alpha: unsafe extern "C" fn(*const Handle) -> c_int,
    decode: unsafe extern "C" fn(
        *const Handle,
        *mut *mut Image,
        c_int,
        c_int,
        *const c_void,
    ) -> HeifError,
    image_release: unsafe extern "C" fn(*const Image),
    width: unsafe extern "C" fn(*const Image, c_int) -> c_int,
    height: unsafe extern "C" fn(*const Image, c_int) -> c_int,
    plane: unsafe extern "C" fn(*const Image, c_int, *mut c_int) -> *const u8,
}

// SAFETY: plain function pointers into a library that stays loaded for the life of the process;
// libheif's functions may be called from any thread on separate contexts.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    /// Search the DLL's own folder for its dependencies (libde265.dll lies next to heif.dll).
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;

    fn candidates() -> Vec<PathBuf> {
        let mut list = Vec::new();
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
        {
            list.push(dir.join("heif.dll"));
            // Tests and `tauri dev` run from target/<profile>[/deps]: the vendored copy.
            for up in dir.ancestors().take(5) {
                list.push(up.join("vendor").join("heif").join("heif.dll"));
            }
        }
        list
    }

    pub fn open() -> Option<*mut c_void> {
        candidates()
            .into_iter()
            .filter(|p| p.is_file())
            .find_map(|path| {
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                // SAFETY: a NUL-terminated wide path; a null result means "not loaded".
                let module = unsafe {
                    LoadLibraryExW(
                        wide.as_ptr(),
                        std::ptr::null_mut(),
                        LOAD_WITH_ALTERED_SEARCH_PATH,
                    )
                };
                (!module.is_null()).then_some(module)
            })
    }

    pub fn symbol(library: *mut c_void, name: &[u8]) -> *mut c_void {
        // SAFETY: `name` is NUL-terminated, `library` a loaded module.
        unsafe { GetProcAddress(library, name.as_ptr()) }
    }
}

#[cfg(unix)]
mod platform {
    use std::ffi::{c_char, c_int, c_void};

    extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(library: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    const RTLD_NOW: c_int = 2;

    pub fn open() -> Option<*mut c_void> {
        let names: &[&[u8]] = &[
            b"libheif.so.1\0",
            b"libheif.so\0",
            b"libheif.1.dylib\0",
            b"libheif.dylib\0",
        ];
        names.iter().find_map(|name| {
            // SAFETY: a NUL-terminated name; a null result means "not loaded".
            let library = unsafe { dlopen(name.as_ptr().cast(), RTLD_NOW) };
            (!library.is_null()).then_some(library)
        })
    }

    pub fn symbol(library: *mut c_void, name: &[u8]) -> *mut c_void {
        // SAFETY: `name` is NUL-terminated, `library` a loaded library.
        unsafe { dlsym(library, name.as_ptr().cast()) }
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    use std::ffi::c_void;
    pub fn open() -> Option<*mut c_void> {
        None
    }
    pub fn symbol(_: *mut c_void, _: &[u8]) -> *mut c_void {
        std::ptr::null_mut()
    }
}

fn load() -> Option<Api> {
    let library = platform::open()?;
    macro_rules! get {
        ($name:literal, $kind:ty) => {{
            let pointer = platform::symbol(library, concat!($name, "\0").as_bytes());
            if pointer.is_null() {
                return None;
            }
            // SAFETY: the symbol of libheif's documented C API with this signature.
            unsafe { std::mem::transmute::<*mut c_void, $kind>(pointer) }
        }};
    }
    // heif_init exists since libheif 1.13 and loads its plugins (libde265); call it when present.
    let init = platform::symbol(library, b"heif_init\0");
    if !init.is_null() {
        // SAFETY: `heif_error heif_init(struct heif_init_params*)`, NULL is allowed.
        let init = unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_void) -> HeifError>(
                init,
            )
        };
        // SAFETY: see above.
        let _ = unsafe { init(std::ptr::null()) };
    }
    Some(Api {
        context_alloc: get!("heif_context_alloc", unsafe extern "C" fn() -> *mut Ctx),
        context_free: get!("heif_context_free", unsafe extern "C" fn(*mut Ctx)),
        read_from_memory: get!("heif_context_read_from_memory_without_copy", ReadFromMemory),
        primary_handle: get!(
            "heif_context_get_primary_image_handle",
            unsafe extern "C" fn(*mut Ctx, *mut *mut Handle) -> HeifError
        ),
        handle_release: get!(
            "heif_image_handle_release",
            unsafe extern "C" fn(*const Handle)
        ),
        has_alpha: get!(
            "heif_image_handle_has_alpha_channel",
            unsafe extern "C" fn(*const Handle) -> c_int
        ),
        decode: get!("heif_decode_image", Decode),
        image_release: get!("heif_image_release", unsafe extern "C" fn(*const Image)),
        width: get!(
            "heif_image_get_width",
            unsafe extern "C" fn(*const Image, c_int) -> c_int
        ),
        height: get!(
            "heif_image_get_height",
            unsafe extern "C" fn(*const Image, c_int) -> c_int
        ),
        plane: get!(
            "heif_image_get_plane_readonly",
            unsafe extern "C" fn(*const Image, c_int, *mut c_int) -> *const u8
        ),
    })
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(load).as_ref()
}

/// Whether libheif could be loaded (on Windows: the DLLs are in place).
pub fn available() -> bool {
    api().is_some()
}

/// What went wrong while rendering a HEIF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifFailure {
    /// libheif is not there (the DLLs are missing).
    NoLibrary,
    /// The file is damaged or not HEIF.
    Broken,
}

/// Releases libheif objects on every way out of [`render`].
struct Guard<'a> {
    api: &'a Api,
    ctx: *mut Ctx,
    handle: *mut Handle,
    image: *mut Image,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        // SAFETY: each pointer is either null or owned by this guard; released once.
        unsafe {
            if !self.image.is_null() {
                (self.api.image_release)(self.image);
            }
            if !self.handle.is_null() {
                (self.api.handle_release)(self.handle);
            }
            if !self.ctx.is_null() {
                (self.api.context_free)(self.ctx);
            }
        }
    }
}

/// The primary image of a HEIF file as PNG (transformations applied, alpha kept).
pub fn render(data: &[u8]) -> Result<Vec<u8>, HeifFailure> {
    let api = api().ok_or(HeifFailure::NoLibrary)?;
    let mut guard = Guard {
        api,
        ctx: std::ptr::null_mut(),
        handle: std::ptr::null_mut(),
        image: std::ptr::null_mut(),
    };
    // SAFETY: libheif's C API as documented; `data` outlives the context (freed by the guard
    // before this function returns), out-pointers point to the guard's fields.
    let (pixels, width, height, alpha) = unsafe {
        guard.ctx = (api.context_alloc)();
        if guard.ctx.is_null() {
            return Err(HeifFailure::Broken);
        }
        let ok = |e: HeifError| {
            if e.code == 0 {
                Ok(())
            } else {
                Err(HeifFailure::Broken)
            }
        };
        ok((api.read_from_memory)(
            guard.ctx,
            data.as_ptr().cast(),
            data.len(),
            std::ptr::null(),
        ))?;
        ok((api.primary_handle)(guard.ctx, &mut guard.handle))?;
        let alpha = (api.has_alpha)(guard.handle) != 0;
        let chroma = if alpha {
            CHROMA_INTERLEAVED_RGBA
        } else {
            CHROMA_INTERLEAVED_RGB
        };
        ok((api.decode)(
            guard.handle,
            &mut guard.image,
            COLORSPACE_RGB,
            chroma,
            std::ptr::null(),
        ))?;
        let width = (api.width)(guard.image, CHANNEL_INTERLEAVED);
        let height = (api.height)(guard.image, CHANNEL_INTERLEAVED);
        let mut stride: c_int = 0;
        let plane = (api.plane)(guard.image, CHANNEL_INTERLEAVED, &mut stride);
        let (Ok(width), Ok(height), Ok(stride)) = (
            usize::try_from(width),
            usize::try_from(height),
            usize::try_from(stride),
        ) else {
            return Err(HeifFailure::Broken);
        };
        let channels = if alpha { 4 } else { 3 };
        if plane.is_null() || width == 0 || height == 0 || stride < width * channels {
            return Err(HeifFailure::Broken);
        }
        let mut pixels = Vec::with_capacity(width * height * channels);
        for row in 0..height {
            let line = std::slice::from_raw_parts(plane.add(row * stride), width * channels);
            pixels.extend_from_slice(line);
        }
        (pixels, width, height, alpha)
    };
    drop(guard);
    let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
        return Err(HeifFailure::Broken);
    };
    let mut png = Cursor::new(Vec::new());
    // Fast compression: a 12 MP photo is shown at once, the PNG never leaves the process.
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::NoFilter)
        .write_image(
            &pixels,
            width,
            height,
            if alpha {
                ExtendedColorType::Rgba8
            } else {
                ExtendedColorType::Rgb8
            },
        )
        .map_err(|_| HeifFailure::Broken)?;
    Ok(png.into_inner())
}
