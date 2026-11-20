# Spiegelmond

A fast image viewer for Windows 10/11, part of the Moonshine Vibecoder family: open a photo and
see it at once. Only the basics, the picture comes first.

> Status: skeleton (IV-01), the family style (IV-11), languages (IV-12), zoom (IV-02) and browsing the folder (IV-03) — one window, opens an image from the
> command line, the open dialog or by dropping it into the window; light and dark theme, Sichel and
> Vollmond. Zoom, folder browsing, EXIF and the rest follow (IV-02…IV-19).

## Installation

Download `Spiegelmond-Setup-<version>.exe` (the artifact `Spiegelmond-windows` of a CI run) and run
it: **Install → Finish**. It installs for the current user into `%LOCALAPPDATA%\Programs\Spiegelmond`,
without administrator rights and without UAC, in the language of Windows (Russian, English or
German; English otherwise). It adds Spiegelmond to Start and the desktop and to "Open with" of
every format it shows. The finish page has a box (not ticked) to add `iv` to your PATH; a silent
install does that with `Spiegelmond-Setup-<version>.exe /S /PATH`.

`Spiegelmond-<version>-portable.exe` runs without installing: put it in any folder, start it, or
drop a photo on it, or `Spiegelmond-<version>-portable.exe photo.jpg`.

**Spiegelmond as the default program for photos.** Windows does not let a program make itself the
default silently. After installing: Settings → Apps → Default apps → Spiegelmond → for each format
(`.jpg`, `.png`, `.heic`…) choose Spiegelmond. Or right-click a photo → Open with → Choose another
app → Spiegelmond → "Always".

**From the command line.** With the PATH box ticked, a new terminal opens images with
`iv photo.jpg` or `iv "D:\Fotos\2026\IMG_0001.HEIC"` (a running Spiegelmond shows it in its window).

### Uninstalling

Settings → Apps → Installed apps → Spiegelmond → Uninstall (or Start → Spiegelmond → "Uninstall
Spiegelmond", or Control Panel → Programs). Two clicks, no UAC, in the language of Windows; a
running Spiegelmond is closed first. Removed: the program, its shortcuts, "Open with" and Default
apps entries, the PATH entry and the thumbnail cache (`%LOCALAPPDATA%\Spiegelmond\cache`). Kept:
your settings (the box "Also delete my settings" removes them too), the journal of metadata changes
(`%LOCALAPPDATA%\Spiegelmond\journal`, your way back) and, of course, your photos. If Spiegelmond
was the default program, Windows asks which program to use the next time you open such a photo.
A Spiegelmond button pinned to the taskbar stops working; Windows removes it or it can be unpinned
with a right click. Installing again afterwards starts from scratch.

### If Windows blocks the installation

The installer is not signed: a code signing certificate costs money, and free ways are still being
checked (MV-12). Windows therefore shows one of two windows. The text below can be forwarded as it is.

**Если Windows блокирует установку (по-русски)**

- *Синее окно «Windows защитила ваш компьютер»* (SmartScreen): нажмите «Подробнее», затем
  «Выполнить в любом случае». Это нормально для программ без платной подписи.
- *Окно «Интеллектуальное управление приложениями заблокировало…»* (Smart App Control): кнопки
  «всё равно запустить» нет, исключений для отдельных программ Windows не делает. Пока программа не
  подписана, поставить её можно только выключив эту функцию: «Пуск → Безопасность Windows →
  Управление приложениями/браузером → Параметры интеллектуального управления приложениями → Выкл.»
  Это снижает защиту компьютера; решайте сами, нужна ли вам эта программа настолько.

**If Windows blocks the installation (English)**

- *Blue window "Windows protected your PC"* (SmartScreen): click "More info", then "Run anyway".
  This is normal for programs without a paid signature.
- *"Smart App Control blocked …"*: there is no "run anyway", and Windows makes no exceptions for
  single programs. Until the program is signed, the only way is to turn the feature off: "Start →
  Windows Security → App & browser control → Smart App Control settings → Off". This lowers the
  protection of the computer; decide whether you need the program that much.

**Wenn Windows die Installation blockiert (Deutsch)**

- *Blaues Fenster „Der Computer wurde durch Windows geschützt"* (SmartScreen): auf „Weitere
  Informationen" klicken, dann auf „Trotzdem ausführen". Das ist bei Programmen ohne kostenpflichtige
  Signatur normal.
- *„Die intelligente App-Steuerung hat … blockiert"* (Smart App Control): Es gibt kein „Trotzdem
  ausführen", und Windows macht keine Ausnahmen für einzelne Programme. Solange das Programm nicht
  signiert ist, geht es nur, wenn die Funktion ausgeschaltet wird: „Start → Windows-Sicherheit →
  App- & Browsersteuerung → Einstellungen für die intelligente App-Steuerung → Aus". Das senkt den
  Schutz des Computers; entscheiden Sie selbst, ob Sie das Programm so dringend brauchen.

The names of the menu items can differ slightly between Windows versions. The program never asks
to turn anything off by itself, and it does not try to get around these checks.

## Where the data is

Settings: `%LOCALAPPDATA%\Spiegelmond\settings.json` (theme, mode, background behind the image,
language, sort order, slideshow interval, film strip); thumbnails: `%LOCALAPPDATA%\Spiegelmond\cache`;
journal of metadata changes: `%LOCALAPPDATA%\Spiegelmond\journal`; the portable build keeps them in `Spiegelmond-data` next to its exe if that folder can
be written. Spiegelmond never changes your files without an explicit command; deleting goes only to
the Recycle Bin (see `CLAUDE.md`, "Safety").

## Keyboard shortcuts

Generated from `src/lib/hotkeys.json` (`node scripts/hotkeys-markdown.mjs --write`); the "?" / F1 help
reads the same table.

<!-- hotkeys:begin (generated by scripts/hotkeys-markdown.mjs) -->
| Key | Action |
|---|---|
| ← → · PgUp PgDn | Previous · next image in the folder |
| Home · End | First · last image |
| wheel | Previous · next image with the mouse wheel |
| Ctrl+wheel · Ctrl++ · Ctrl+− | Zoom in · out (Ctrl+wheel and pinch — to the cursor) |
| Ctrl+0 | Fit to the window |
| 1 | Actual size (100 %) |
| W | Fit to the width |
| double-click | Fit ↔ 100 % at the click |
| drag · ← → ↑ ↓ | Move an enlarged image |
| F · F11 | Full screen (Esc — leave) |
| Space | Slideshow: start · pause |
| B | Background behind the image: theme · black · checkerboard |
| R · Shift+R | Rotate right · left (view only) |
| Ctrl+S | Write the view rotation into the file (with a preview) |
| I | Information panel |
| D | Set the date taken by hand (preview, undo) |
| T | Thumbnail strip |
| Delete | Delete to the Recycle Bin |
| Ctrl+Z | Undo the deletion |
| Ctrl+C | Copy the image |
| Ctrl+Shift+C | Copy the path |
| Ctrl+O | Open an image |
| Ctrl+M | Switch between Sichel and Vollmond |
| ? · F1 | These keyboard shortcuts |
| Esc | Close a dialog · leave full screen |
<!-- hotkeys:end -->

Opening: `iv path\to\photo.jpg`, a double-click on a file once Spiegelmond is the default viewer
(IV-10), or drag and drop into the window. A second start hands the file to the open window.

Only the image is on screen: the top bar (open, mode, Vollmond tools, language, settings) appears
when the mouse moves and hides after two seconds. Settings: theme (System / Light / Dark), the
background behind the image (as the theme — indigo or silver —, black, checkerboard), language,
About.

Languages: English, Russian, German (`locales/`, MV/I18N.md); the first start follows Windows,
the dropdown in the top bar switches at once. Screenshots: `docs/screenshots/iv-12/`.

Formats: shown by WebView2 itself — JPG, PNG, GIF (animated), WebP (animated), BMP, AVIF; rendered
by `iv-core` and served through the `ivimg` protocol — TIFF (the first page, its orientation
applied) and camera RAW built on TIFF (DNG, CR2, NEF, NRW, ARW, SRW, ORF, RW2, PEF, ERF, 3FR: the
largest embedded JPEG preview, turned like the RAW says). A damaged file shows a placeholder with
its name; browsing goes on. HEIC / HEIF follow (libheif, IV-05).

## Browsing a folder

Opening an image shows it at once; the other images of its folder are listed in the background
(by name in natural order — 2 before 10 —, by date modified, or by date taken once IV-04 reads
EXIF; the order is a setting). There is no wrap-around: at the ends a short hint appears. The two
images on each side are decoded ahead, within a memory budget (`cacheMb`, 512 MB by default,
least recently used out first). Files added or removed meanwhile are picked up on the next
step; a vanished file is skipped.

**T** shows the film strip: thumbnails of the folder along the bottom, the current one lit and in
the middle, a click goes there. Only the thumbnails in view exist in the page, so a folder of
10 000 photos scrolls as smoothly as one of 10. Thumbnails come from the photo's EXIF thumbnail
when it has one, otherwise from a scaled copy; they are built in the background (what is on screen
first) and kept on disk (`%LOCALAPPDATA%\Spiegelmond\cache\thumbs`, at most 256 MB; a changed
photo gets a new thumbnail).

## Vollmond: tools for a photo archive (IV-13)

Ctrl+M switches to Vollmond; the Tools button opens its window (loaded only then, so the viewer
starts as fast as without it). Choose the folder of the archive (with or without subfolders) and
**Index**: in the background, with progress and **Stop**, Spiegelmond reads which images are there
and their EXIF, then a fingerprint of each file's content (for duplicates), then the thumbnails
(the same cache as the film strip). Nothing is changed in the photos. Opening the same folder again
is instant: only new and changed files are read again (50 000 files: 3.7 s the first time, 0.13 s
again, measured with tiny synthetic files). The index lives in `%LOCALAPPDATA%\Spiegelmond\cache`.

Every tool that will change files (dates, duplicates, batch processing: IV-14…IV-19) works the
same safe way: a preview "before → after" with totals first; at most 500 files at once, and for
100 files or more a trial batch of 10 first; a warning for folders synced to a cloud (OneDrive,
kDrive, Google Drive, Dropbox…) or on the network; the old values go to the journal of changes
(`%LOCALAPPDATA%\Spiegelmond\journal\operations.jsonl`) before a file is touched; each file is
written next to the original, checked (the picture must stay exactly the same, the new values must
read back) and only then put in place; the first problem stops the batch. The journal is shown in
the Tools window; undoing an operation applies the old values back the same safe way (each tool
adds its Undo button there).

### Dates (IV-14)

After indexing, **Show the dates**: for every photo the date taken (EXIF), the date in its file
name (Xiaomi and other Android cameras `IMG_20230512_143000`, Pixel `PXL_…`, WhatsApp
`IMG-20230512-WA0001`, Telegram `photo_2023-05-12_14-30-00`, screenshots, Dropbox
`2023-05-12 14.30.00`) and the file date that phone galleries sort by. Mismatches are marked; the
filters show all files, only mismatches or files without a date taken. **Preview the fixes** shows
what would change: a missing date taken is added from the name (JPEG; written so that every other
piece of metadata and the picture stay exactly as they were), the file date is set to the date
taken. An existing date taken is never changed; dates in the future or before 1990 are left out
unless you tick them. Then the batches are applied one by one, each after "Yes, change", and every
batch can be undone from the journal.

## Performance (IV-09)

Measured 2026-11-20 in the cloud: Linux, headless Chromium (the engine of WebView2) **without a
GPU**, so drawing is done in software — a Windows PC with a graphics card is at least as fast.

| What | Result |
|---|---|
| Listing a folder, 5 000 / 20 000 files (`cargo test -p iv-core --release -- --ignored --nocapture`) | ~11 ms / ~50 ms |
| Opening a 50 / 100 / 200 MP JPEG until it is on screen | ~0.2 s each |
| Zooming (Ctrl+wheel, to 32×) and panning a 50 MP photo | 60 frames/s; one 83 ms pause at the first zoom step |
| The same, 100 MP | 60 frames/s; one 133 ms pause at the first zoom step |
| The same, 200 MP | 60 frames/s between pauses of 1–1.5 s (software drawing; to be checked on Windows) |
| Film strip of 10 000 files, fast scrolling | 60 frames/s (median frame 16.7 ms, slowest 17.7 ms) |
| Memory over 1 000 images in a row (12 MP each, ±2 decoded ahead, 512 MB budget) | the cache stays at 504 MB; the whole browser grows to ~1.3 GB by image 700 and stays there — no leak |

## Writing a rotation into a file

Only on an explicit Ctrl+S with a preview ("now → after", the number of files). For a JPEG with an
EXIF orientation tag only the 2 bytes of that tag change: the picture, camera data, GPS, colour
profile and Maker Notes stay byte for byte as they were (checked after writing, not assumed). The
old value goes to the journal (`%LOCALAPPDATA%\Spiegelmond\journal\journal.jsonl`) before the file
is touched; the new file is written next to the original, checked and only then put in its place,
and checked again. "Undo" in the notice writes the old value back the same way. Files without the
tag (and other formats) are refused, unchanged. `testdata/orientation-1…8.jpg` show an upright F
in every orientation.

## Build

Requirements: Node.js 22, Rust (stable), on Linux the Tauri system libraries
(`libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`).

```
npm ci
npm run tauri dev      # run in development mode (npm run tauri dev -- -- path\to\photo.jpg)
npm run tauri build    # Windows: target/release/bundle/nsis/*-setup.exe and target/release/iv.exe
```

Icons: `scripts/make-icons.sh` rebuilds `src-tauri/icons` from `assets/brand/icon.svg` (32 px and
up) and `assets/brand/icon-small.svg` (16 and 24 px, without craters and glow). The look of the
interface: screenshots in `docs/screenshots/iv-11/` (two themes × two modes, dialogs, splash).
In the browser preview (`npm run dev`, then `http://localhost:1421/?demo`) a demo picture is shown.

## Tests

```
cargo test -p iv-core    # image logic, any OS, no GUI
cargo test --workspace   # all Rust tests
npm test                 # interface logic (Vitest)
npm run lint             # tsc --noEmit, cargo fmt --check, cargo clippy -D warnings
python3 scripts/make-testdata.py   # regenerate testdata/
```

Tests use only the files of `testdata/` (generated by `scripts/make-testdata.py`), copied into a
temporary folder.

CI (GitHub Actions): Linux — lint, `iv-core` and interface tests; Windows — workspace tests,
installer and portable exe as the artifact `Spiegelmond-windows`.
