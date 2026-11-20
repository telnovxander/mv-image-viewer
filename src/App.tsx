import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import demoImage from "../assets/brand/splash-dark.jpg";
import { AboutDialog } from "./components/AboutDialog";
import { HelpDialog } from "./components/HelpDialog";
import { ImageView } from "./components/ImageView";
import { DateEditDialog } from "./components/DateEditDialog";
import { Filmstrip } from "./components/Filmstrip";
import { ImageMenu, type ImageAction } from "./components/ImageMenu";
import { InfoPanel } from "./components/InfoPanel";
import { WriteRotationDialog } from "./components/WriteRotationDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import { ToolsDialog } from "./components/ToolsDialog";
import { TopBar } from "./components/TopBar";
import { applyBatch, undoPlan, applyRotation, copyImage, copyPath, exifOf, heifAvailable, openWith, restoreFile, revealFile, trashFile, type Trashed, imageUrl, inTauri, initialPath, isBackendError, listFolder, openImage, openMap, pathUrl, planRotation, undoRotation, type ExifInfo, type FolderEntry, type ImageFile, type JournalEntry, type OrientationPlan } from "./lib/backend";
import { rotate, type Rotation, type Size } from "./lib/zoom";
import { afterRemoval, relocate, step, type Edge } from "./lib/browse";
import { ImageCache, neighbours } from "./lib/preload";
import { CHROME_HIDE_MS, chromeVisible } from "./lib/chrome";
import { DECODED_EXTENSIONS, isHeif, WEB_EXTENSIONS } from "./lib/formats";
import { BACKGROUNDS, loadSettings, saveSettings, type Settings, type SortOrder } from "./lib/settings";
import { applyAppearance, resolveTheme, toggleMode } from "./lib/theme";
import { windowTitle } from "./lib/title";

type DialogName = "settings" | "about" | "tools" | "help" | "write" | null | "date";

const DARK_QUERY = "(prefers-color-scheme: dark)";
/** The folder is read again on browsing at most this often, to pick up added or removed files. */
const REFRESH_MS = 2000;
const EDGE_HINT_MS = 1200;

/** Browser preview only (`npm run dev`, `?demo`): a picture to look at the style without Tauri;
 * `?demo&image=<url>` shows another one (IV-09: timing of 50–200 MP photos in Chromium). */
const DEMO_PARAMS = new URLSearchParams(window.location.search);
const DEMO: ImageFile | null =
  !inTauri && DEMO_PARAMS.has("demo")
    ? { path: DEMO_PARAMS.get("image") ?? demoImage, name: "spiegelmond-demo.jpg", size: 412_345, format: "web" }
    : null;
/** Preview only (`?demo&folder=10000`): a folder of that many copies of the demo picture, to try
 * the film strip with a big folder (IV-08). */
const DEMO_FOLDER: FolderEntry[] = DEMO
  ? Array.from({ length: Number(DEMO_PARAMS.get("folder") ?? 0) }, (_, i) => ({
      path: `${demoImage}#${i}`,
      name: `IMG_${String(i + 1).padStart(5, "0")}.jpg`,
      size: 412_345,
      modified: 0,
    }))
  : [];
/** Preview only: EXIF to look at the information panel without Tauri. */
const DEMO_EXIF: ExifInfo | null = DEMO
  ? {
      orientation: 1,
      taken: "2023-05-12T21:47:00",
      camera: "Canon EOS R6",
      lens: "RF24-105mm F4 L IS USM",
      exposure: 1 / 60,
      f_number: 4,
      iso: 3200,
      focal_length: 35,
      gps: [47.37, 8.54],
      width: 1312,
      height: 816,
    }
  : null;

export function App() {
  const { t, i18n } = useTranslation();
  const [image, setImage] = useState<ImageFile | null>(DEMO);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const [systemDark, setSystemDark] = useState(() => window.matchMedia(DARK_QUERY).matches);
  const [dialog, setDialog] = useState<DialogName>(null);
  const [recentlyMoved, setRecentlyMoved] = useState(false);
  const [overBar, setOverBar] = useState(false);
  const hideTimer = useRef<number | undefined>(undefined);
  const [entries, setEntries] = useState<FolderEntry[]>(DEMO_FOLDER);
  const [rotation, setRotation] = useState<Rotation>(0);
  const [shownSize, setShownSize] = useState<Size | null>(null);
  const [exif, setExif] = useState<ExifInfo | null>(DEMO_EXIF);
  const [infoOpen, setInfoOpen] = useState(false);
  /** The current image could not be decoded: a placeholder with its name instead (IV-05). */
  const [broken, setBroken] = useState(false);
  // A HEIC that cannot be shown: decoder missing (true) or the file is damaged (false).
  const [noCodec, setNoCodec] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [slideshow, setSlideshow] = useState(false);
  const [plan, setPlan] = useState<OrientationPlan | null>(null);
  /** The last write, offered for undo; `undone` once it was undone. */
  const [written, setWritten] = useState<{ entry: JournalEntry; undone: boolean } | null>(null);
  /** Changes when a file was rewritten, so the WebView loads it again. */
  const [version, setVersion] = useState(0);
  /** The last file moved to the Recycle Bin, for Ctrl+Z (IV-07). */
  const [trashed, setTrashed] = useState<Trashed | null>(null);
  /** A short message at the bottom (deleted, copied, "only to the Recycle Bin"). */
  const [notice, setNotice] = useState<{ text: string; undo: "trash" | "date" | null } | null>(null);
  /** The journal operation of the last date set by hand (IV-15), for undo from the notice. */
  const [dateOp, setDateOp] = useState<number | null>(null);
  const noticeTimer = useRef<number>(0);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [index, setIndex] = useState(0);
  const [edge, setEdge] = useState<Edge>(null);
  const edgeTimer = useRef<number | undefined>(undefined);
  const lastRefresh = useRef(0);
  const cache = useRef<ImageCache | null>(null);
  if (cache.current === null) cache.current = new ImageCache(settings.cacheMb * 1024 * 1024);

  const update = useCallback((change: (s: Settings) => Partial<Settings>) => {
    setSettings((s) => {
      const next = { ...s, ...change(s) };
      saveSettings(next);
      return next;
    });
  }, []);

  useEffect(() => {
    const query = window.matchMedia(DARK_QUERY);
    const onChange = () => setSystemDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    applyAppearance(resolveTheme(settings.theme, systemDark), settings.mode);
  }, [settings.theme, settings.mode, systemDark]);

  const changeLanguage = useCallback(
    (code: string) => {
      void i18n.changeLanguage(code);
      update(() => ({ language: code }));
    },
    [i18n, update],
  );

  /** Read the folder of `file` again and find `file` in it (or the image that took its place). */
  const refresh = useCallback(async (file: string, oldIndex: number, order: SortOrder = loadSettings().sort) => {
    lastRefresh.current = Date.now();
    try {
      const list = await listFolder(file, order);
      const at = relocate(
        list.map((e) => e.path),
        file,
        oldIndex,
      );
      setEntries(list);
      setIndex(at);
      return { list, at };
    } catch {
      return null;
    }
  }, []);

  const preloadAround = useCallback((list: FolderEntry[], at: number, direction: 1 | -1) => {
    const store = cache.current;
    if (!store || !inTauri) return;
    store.touch(pathUrl(list[at]?.path ?? ""));
    for (const i of neighbours(at, list.length, direction)) store.preload(pathUrl(list[i].path));
  }, []);

  const open = useCallback(
    async (path: string) => {
      try {
        const file = await openImage(path);
        setImage(file);
        setError(null);
        setEntries([]);
        setIndex(0);
        // The image is on screen; its folder is read in the background.
        const found = await refresh(file.path, 0);
        if (found) preloadAround(found.list, found.at, 1);
      } catch (e) {
        setError(isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : t("error.other", { path }));
      }
    },
    [preloadAround, refresh, t],
  );

  const showEdge = useCallback((which: Edge) => {
    setEdge(which);
    window.clearTimeout(edgeTimer.current);
    edgeTimer.current = window.setTimeout(() => setEdge(null), EDGE_HINT_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(edgeTimer.current), []);

  /** Show the image at `target` of the folder list; a vanished file makes the list be read again. */
  const browseTo = useCallback(
    async (target: number, direction: 1 | -1) => {
      let list = entries;
      let at = target;
      for (let attempt = 0; attempt < 3 && list.length > 0; attempt++) {
        const entry = list[at];
        try {
          const file = await openImage(entry.path);
          setImage(file);
          setIndex(at);
          setError(null);
          preloadAround(list, at, direction);
          if (Date.now() - lastRefresh.current > REFRESH_MS) void refresh(file.path, at);
          return;
        } catch (e) {
          if (!isBackendError(e) || e.code !== "not_found") {
            setError(isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : t("error.other", { path: entry.path }));
            return;
          }
          const found = await refresh(entry.path, at);
          if (!found) return;
          list = found.list;
          at = found.at;
        }
      }
    },
    [entries, preloadAround, refresh, t],
  );

  const go = useCallback(
    (delta: number | "first" | "last") => {
      if (entries.length === 0) return;
      const result =
        delta === "first" ? step(index, entries.length, -entries.length) : delta === "last" ? step(index, entries.length, entries.length) : step(index, entries.length, delta);
      if (result.edge) showEdge(result.edge);
      if (result.index !== index) void browseTo(result.index, result.index > index ? 1 : -1);
    },
    [browseTo, entries.length, index, showEdge],
  );

  const changeSort = useCallback(
    (sort: SortOrder) => {
      update(() => ({ sort }));
      if (image) void refresh(image.path, index, sort);
    },
    [image, index, refresh, update],
  );

  const chooseFile = useCallback(async () => {
    if (!inTauri) return;
    const path = await openDialog({
      title: t("open.dialog_title"),
      multiple: false,
      directory: false,
      filters: [{ name: t("open.filter"), extensions: [...WEB_EXTENSIONS, ...DECODED_EXTENSIONS] }],
    });
    if (typeof path === "string") await open(path);
  }, [open, t]);

  // The file of the command line, files of later starts (single instance) and dropped files.
  useEffect(() => {
    if (!inTauri) return;
    void initialPath().then((path) => {
      if (path) void open(path);
    });
    const unlisten = [
      listen<string>("open-path", (event) => void open(event.payload)),
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) void open(event.payload.paths[0]);
      }),
    ];
    return () => unlisten.forEach((p) => void p.then((stop) => stop()));
  }, [open]);

  const toggleFullscreen = useCallback(async () => {
    const next = !fullscreen;
    setFullscreen(next);
    if (inTauri) await getCurrentWindow().setFullscreen(next);
    else if (next) await document.documentElement.requestFullscreen?.().catch(() => undefined);
    else if (document.fullscreenElement) await document.exitFullscreen().catch(() => undefined);
  }, [fullscreen]);

  const errorText = useCallback(
    (e: unknown, path: string) => (isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : t("error.other", { path })),
    [t],
  );

  /** Ctrl+S: preview of writing the view rotation into the file; nothing is changed yet. */
  const previewWrite = useCallback(async () => {
    if (!image || rotation === 0 || !inTauri) return;
    try {
      setPlan(await planRotation(image.path, rotation));
      setDialog("write");
    } catch (e) {
      setError(errorText(e, image.path));
    }
  }, [errorText, image, rotation]);

  const reload = useCallback(async (path: string) => {
    setRotation(0);
    setVersion((v) => v + 1);
    setExif(await exifOf(path));
  }, []);

  const applyWrite = useCallback(async () => {
    if (!plan) return;
    setDialog(null);
    try {
      const entry = await applyRotation(plan);
      setWritten({ entry, undone: false });
      await reload(plan.path);
    } catch (e) {
      setError(errorText(e, plan.path));
    }
    setPlan(null);
  }, [errorText, plan, reload]);

  const undoWrite = useCallback(async () => {
    if (!written || written.undone) return;
    try {
      await undoRotation(written.entry);
      setWritten({ ...written, undone: true });
      await reload(written.entry.path);
    } catch (e) {
      setError(errorText(e, written.entry.path));
    }
  }, [errorText, reload, written]);

  const showNotice = useCallback((text: string, undo: "trash" | "date" | null = null) => {
    setNotice({ text, undo });
    window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(null), 6000);
  }, []);
  useEffect(() => () => window.clearTimeout(noticeTimer.current), []);

  /**
   * Delete: the image goes to the Recycle Bin, never past it, without asking, and the next one
   * is shown (IV/PROJECT.md, safety points 1–4). Where the bin does not work, nothing happens and
   * the message says why.
   */
  const deleteCurrent = useCallback(async () => {
    if (!image || !inTauri) return;
    const path = image.path;
    try {
      const done = await trashFile(path);
      setTrashed(done);
      setSlideshow(false);
      showNotice(t("delete.done", { name: image.name }), "trash");
      const list = entries.filter((e) => e.path !== path);
      const next = afterRemoval(index, entries.length);
      setEntries(list);
      setIndex(Math.max(0, next));
      if (next < 0 || list.length === 0) {
        setImage(null);
        return;
      }
      try {
        setImage(await openImage(list[next].path));
        preloadAround(list, next, 1);
      } catch {
        // The next file vanished meanwhile: read the folder again.
        void refresh(list[next].path, next);
      }
    } catch (e) {
      setError(errorText(e, path));
    }
  }, [entries, errorText, image, index, preloadAround, refresh, showNotice, t]);

  /** Ctrl+Z after Delete: the file comes back from the Recycle Bin and is shown again. */
  const undoDelete = useCallback(async () => {
    if (!trashed) return;
    try {
      await restoreFile(trashed);
      setTrashed(null);
      showNotice(t("delete.restored"));
      await open(trashed.path);
    } catch (e) {
      setError(errorText(e, trashed.path));
    }
  }, [errorText, open, showNotice, t, trashed]);

  const copyCurrent = useCallback(async () => {
    if (!image || !inTauri) return;
    try {
      await copyImage(image.path, rotation);
      showNotice(t("copy.image_done"));
    } catch (e) {
      setError(errorText(e, image.path));
    }
  }, [errorText, image, rotation, showNotice, t]);

  const copyCurrentPath = useCallback(async () => {
    if (!image || !inTauri) return;
    try {
      await copyPath(image.path);
      showNotice(t("copy.path_done"));
    } catch (e) {
      setError(errorText(e, image.path));
    }
  }, [errorText, image, showNotice, t]);

  const runImageAction = useCallback(
    (action: ImageAction) => {
      if (!image || !inTauri) return;
      if (action === "copy_image") void copyCurrent();
      else if (action === "copy_path") void copyCurrentPath();
      else if (action === "delete") void deleteCurrent();
      else if (action === "reveal") revealFile(image.path).catch((e: unknown) => setError(errorText(e, image.path)));
      else openWith(image.path).catch((e: unknown) => setError(errorText(e, image.path)));
    },
    [copyCurrent, copyCurrentPath, deleteCurrent, errorText, image],
  );

  /** Undo the last date set by hand: the old values back through the same safe write. */
  const undoDate = useCallback(async () => {
    if (dateOp === null || !image) return;
    try {
      const plan = await undoPlan(dateOp);
      const report = await applyBatch(plan, 0, plan.changes.length, dateOp);
      if (report.error) setError(errorText(report.error, image.path));
      else showNotice(t("date_edit.undone"));
      setDateOp(null);
      setExif(await exifOf(image.path));
    } catch (e) {
      setError(errorText(e, image.path));
    }
  }, [dateOp, errorText, image, showNotice, t]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "F1" || (event.key === "?" && !(event.target instanceof HTMLInputElement))) {
        event.preventDefault();
        setDialog((current) => current ?? "help");
        return;
      }
      if (event.defaultPrevented) return;
      if (event.key === "Delete" && !event.ctrlKey && !event.altKey) {
        event.preventDefault();
        // Shift+Delete means "past the Recycle Bin" in Windows: Spiegelmond never does that.
        if (event.shiftKey) showNotice(t("delete.only_to_bin"));
        else void deleteCurrent();
        return;
      }
      if (!event.ctrlKey && !event.altKey && !event.shiftKey) {
        const moves: Record<string, number | "first" | "last"> = { ArrowRight: 1, ArrowLeft: -1, PageDown: 1, PageUp: -1, Home: "first", End: "last" };
        if (event.key in moves) {
          event.preventDefault();
          go(moves[event.key]);
          return;
        }
      }
      if (!event.ctrlKey && !event.altKey && !(event.target instanceof HTMLSelectElement)) {
        if (event.key === "F11" || event.key === "f" || event.key === "F") {
          event.preventDefault();
          void toggleFullscreen();
          return;
        }
        if (event.key === "Escape" && (fullscreen || slideshow)) {
          event.preventDefault();
          setSlideshow(false);
          if (fullscreen) void toggleFullscreen();
          return;
        }
        if (event.key === " ") {
          event.preventDefault();
          if (entries.length > 1) setSlideshow((on) => !on);
          return;
        }
        if (event.key === "b" || event.key === "B") {
          event.preventDefault();
          update((s) => ({ background: BACKGROUNDS[(BACKGROUNDS.indexOf(s.background) + 1) % BACKGROUNDS.length] }));
          return;
        }
        if ((event.key === "d" || event.key === "D") && image && inTauri) {
          // IV-15: the date of the current photo by hand (also the button in the I panel).
          event.preventDefault();
          setDialog((current) => current ?? "date");
          return;
        }
        if (event.key === "t" || event.key === "T") {
          event.preventDefault();
          update((s) => ({ thumbnails: !s.thumbnails }));
          return;
        }
        if (event.key === "i" || event.key === "I") {
          event.preventDefault();
          setInfoOpen((open) => !open);
          return;
        }
        if (event.key === "r" || event.key === "R") {
          event.preventDefault();
          setRotation((r) => rotate(r, !event.shiftKey));
          return;
        }
      }
      if (!event.ctrlKey || event.altKey) return;
      const key = event.key.toLowerCase();
      if (key === "z" && !event.shiftKey) {
        event.preventDefault();
        void undoDelete();
      } else if (key === "c") {
        if (window.getSelection()?.toString()) return;
        event.preventDefault();
        void (event.shiftKey ? copyCurrentPath() : copyCurrent());
      } else if (key === "o") {
        event.preventDefault();
        void chooseFile();
      } else if (key === "s") {
        event.preventDefault();
        void previewWrite();
      } else if (key === "m") {
        event.preventDefault();
        update((s) => ({ mode: toggleMode(s.mode) }));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [chooseFile, copyCurrent, copyCurrentPath, deleteCurrent, entries.length, image, fullscreen, go, previewWrite, showNotice, slideshow, t, toggleFullscreen, undoDelete, update]);

  // Slideshow: the next image every slideSeconds; it stops on the last one.
  useEffect(() => {
    if (!slideshow) return;
    if (index >= entries.length - 1) {
      setSlideshow(false);
      return;
    }
    const timer = window.setTimeout(() => go(1), settings.slideSeconds * 1000);
    return () => window.clearTimeout(timer);
  }, [entries.length, go, index, settings.slideSeconds, slideshow]);

  // The bars appear on a mouse move and hide again after a pause.
  const onMouseMove = useCallback(() => {
    setRecentlyMoved(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setRecentlyMoved(false), CHROME_HIDE_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(hideTimer.current), []);

  useEffect(() => {
    const count = Math.max(entries.length, image ? 1 : 0);
    const title = windowTitle(image?.name ?? null, entries.length > 0 ? index + 1 : 1, count, i18n.language);
    document.title = title;
    if (inTauri) void getCurrentWindow().setTitle(title);
  }, [image, entries.length, index, i18n.language]);

  // A new image: no view rotation, its own EXIF.
  useEffect(() => {
    setRotation(0);
    setShownSize(null);
    setBroken(false);
    setNoCodec(false);
    if (!image || !inTauri) return;
    let current = true;
    void exifOf(image.path).then((info) => {
      if (current) setExif(info);
    });
    return () => {
      current = false;
    };
  }, [image]);

  const visible = chromeVisible({ recentlyMoved, overBar, dialogOpen: dialog !== null, empty: image === null });
  const toggle = () => update((s) => ({ mode: toggleMode(s.mode) }));

  return (
    <div className={`app stage stage--${settings.background}${visible ? "" : " app--idle"}`} onMouseMove={onMouseMove}>
      <TopBar
        mode={settings.mode}
        visible={visible}
        onOpen={() => void chooseFile()}
        onToggleMode={toggle}
        onTools={() => setDialog("tools")}
        onSettings={() => setDialog("settings")}
        onHelp={() => setDialog("help")}
        onLanguage={changeLanguage}
        onHover={setOverBar}
      />
      <main
        className="viewer"
        onContextMenu={(event) => {
          event.preventDefault();
          if (image && inTauri) setMenu({ x: event.clientX, y: event.clientY });
        }}
      >
        {image && (
          <ImageView
            key={image.path}
            src={version > 0 ? `${imageUrl(image)}?v=${version}` : imageUrl(image)}
            onWheelPage={go}
            rotation={rotation}
            alt={t("image.alt", { name: image.name })}
            onError={() => {
              setBroken(true);
              const path = image.path;
              if (isHeif(path)) void heifAvailable().then((ok) => setNoCodec(!ok));
            }}
            onLoad={(size) => {
              setError(null);
              setBroken(false);
              setShownSize(size);
            }}
          />
        )}
        {image && infoOpen && <InfoPanel file={image} size={shownSize} exif={exif} onOpenMap={(lat, lon) => void openMap(lat, lon)} onEditDate={inTauri ? () => setDialog("date") : undefined} />}
        {image && broken && (
          <div className="viewer__empty" role="alert">
            <p className="viewer__empty-title">{image.name}</p>
            <p className="viewer__empty-hint">{t(noCodec ? "error.no_codec" : "error.decode", { name: image.name })}</p>
          </div>
        )}
        {!image && !error && (
          <div className="viewer__empty">
            <p className="viewer__empty-title">{t("empty.title")}</p>
            <p className="viewer__empty-hint">{t("empty.hint")}</p>
          </div>
        )}
        {settings.thumbnails && entries.length > 0 && (
          <Filmstrip entries={entries} index={index} onChoose={(at) => at !== index && void browseTo(at, at > index ? 1 : -1)} />
        )}
        {edge && (
          <p className="edge-hint" role="status">
            {t(edge === "first" ? "browse.first" : "browse.last")}
          </p>
        )}
        {error && (
          <p className="viewer__error" role="alert">
            {error}
          </p>
        )}
      </main>
      {dialog === "settings" && (
        <SettingsDialog
          theme={settings.theme}
          background={settings.background}
          sort={settings.sort}
          onSortChange={changeSort}
          slideSeconds={settings.slideSeconds}
          onSlideSecondsChange={(slideSeconds) => update(() => ({ slideSeconds }))}
          onThemeChange={(theme) => update(() => ({ theme }))}
          onBackgroundChange={(background) => update(() => ({ background }))}
          onLanguage={changeLanguage}
          onAbout={() => setDialog("about")}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "about" && <AboutDialog onClose={() => setDialog(null)} />}
      {dialog === "tools" && (
        <ToolsDialog
          protectedFolders={settings.protectedFolders}
          onProtectedChange={(protectedFolders) => update(() => ({ protectedFolders }))}
          presets={settings.batchPresets}
          onPresetsChange={(batchPresets) => update(() => ({ batchPresets }))}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "help" && <HelpDialog onClose={() => setDialog(null)} />}
      {dialog === "date" && image && (
        <DateEditDialog
          path={image.path}
          name={image.name}
          taken={exif?.taken ?? null}
          onDone={(op) => {
            setDialog(null);
            setDateOp(op);
            showNotice(t("date_edit.done"), "date");
            void exifOf(image.path).then(setExif);
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "write" && plan && image && (
        <WriteRotationDialog plan={plan} name={image.name} onApply={() => void applyWrite()} onClose={() => setDialog(null)} />
      )}
      {menu && <ImageMenu x={menu.x} y={menu.y} onChoose={runImageAction} onClose={() => setMenu(null)} />}
      {notice && !written && (
        <div className="toast" role="status">
          <span>{notice.text}</span>
          {notice.undo === "trash" && trashed && (
            <button type="button" className="link-button" onClick={() => void undoDelete()}>
              {t("delete.undo")}
            </button>
          )}
          {notice.undo === "date" && dateOp !== null && (
            <button type="button" className="link-button" onClick={() => void undoDate()}>
              {t("tools.undo")}
            </button>
          )}
          <button type="button" className="icon-button" onClick={() => setNotice(null)} aria-label={t("common.close")} title={t("common.close")}>
            ×
          </button>
        </div>
      )}
      {written && (
        <div className="toast" role="status">
          <span>{t(written.undone ? "write_rotation.undone" : "write_rotation.done")}</span>
          {!written.undone && (
            <button type="button" className="link-button" onClick={() => void undoWrite()}>
              {t("write_rotation.undo")}
            </button>
          )}
          <button type="button" className="icon-button" onClick={() => setWritten(null)} aria-label={t("common.close")} title={t("common.close")}>
            ×
          </button>
        </div>
      )}
    </div>
  );
}
