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
import { SettingsDialog } from "./components/SettingsDialog";
import { ToolsDialog } from "./components/ToolsDialog";
import { TopBar } from "./components/TopBar";
import { imageUrl, inTauri, initialPath, isBackendError, listFolder, openImage, pathUrl, type FolderEntry, type ImageFile } from "./lib/backend";
import { relocate, step, type Edge } from "./lib/browse";
import { ImageCache, neighbours } from "./lib/preload";
import { CHROME_HIDE_MS, chromeVisible } from "./lib/chrome";
import { WEB_EXTENSIONS } from "./lib/formats";
import { loadSettings, saveSettings, type Settings, type SortOrder } from "./lib/settings";
import { applyAppearance, resolveTheme, toggleMode } from "./lib/theme";
import { windowTitle } from "./lib/title";

type DialogName = "settings" | "about" | "tools" | "help" | null;

const DARK_QUERY = "(prefers-color-scheme: dark)";
/** The folder is read again on browsing at most this often, to pick up added or removed files. */
const REFRESH_MS = 2000;
const EDGE_HINT_MS = 1200;

/** Browser preview only (`npm run dev`, `?demo`): a picture to look at the style without Tauri. */
const DEMO: ImageFile | null =
  !inTauri && new URLSearchParams(window.location.search).has("demo")
    ? { path: demoImage, name: "spiegelmond-demo.jpg", size: 0, format: "web" }
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
  const [entries, setEntries] = useState<FolderEntry[]>([]);
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
      filters: [{ name: t("open.filter"), extensions: WEB_EXTENSIONS }],
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

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "F1" || (event.key === "?" && !(event.target instanceof HTMLInputElement))) {
        event.preventDefault();
        setDialog((current) => current ?? "help");
        return;
      }
      if (event.defaultPrevented) return;
      if (!event.ctrlKey && !event.altKey && !event.shiftKey) {
        const moves: Record<string, number | "first" | "last"> = { ArrowRight: 1, ArrowLeft: -1, PageDown: 1, PageUp: -1, Home: "first", End: "last" };
        if (event.key in moves) {
          event.preventDefault();
          go(moves[event.key]);
          return;
        }
      }
      if (!event.ctrlKey || event.altKey) return;
      const key = event.key.toLowerCase();
      if (key === "o") {
        event.preventDefault();
        void chooseFile();
      } else if (key === "m") {
        event.preventDefault();
        update((s) => ({ mode: toggleMode(s.mode) }));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [chooseFile, go, update]);

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
      <main className="viewer">
        {image && (
          <ImageView
            key={image.path}
            src={imageUrl(image)}
            onWheelPage={go}
            alt={t("image.alt", { name: image.name })}
            onError={() => setError(t("error.decode", { name: image.name }))}
            onLoad={() => setError(null)}
          />
        )}
        {!image && !error && (
          <div className="viewer__empty">
            <p className="viewer__empty-title">{t("empty.title")}</p>
            <p className="viewer__empty-hint">{t("empty.hint")}</p>
          </div>
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
          onThemeChange={(theme) => update(() => ({ theme }))}
          onBackgroundChange={(background) => update(() => ({ background }))}
          onLanguage={changeLanguage}
          onAbout={() => setDialog("about")}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "about" && <AboutDialog onClose={() => setDialog(null)} />}
      {dialog === "tools" && <ToolsDialog onClose={() => setDialog(null)} />}
      {dialog === "help" && <HelpDialog onClose={() => setDialog(null)} />}
    </div>
  );
}
