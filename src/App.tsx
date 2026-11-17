import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import demoImage from "../assets/brand/splash-dark.jpg";
import { AboutDialog } from "./components/AboutDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import { ToolsDialog } from "./components/ToolsDialog";
import { TopBar } from "./components/TopBar";
import { imageUrl, inTauri, initialPath, isBackendError, openImage, type ImageFile } from "./lib/backend";
import { CHROME_HIDE_MS, chromeVisible } from "./lib/chrome";
import { WEB_EXTENSIONS } from "./lib/formats";
import { loadSettings, saveSettings, type Settings } from "./lib/settings";
import { applyAppearance, resolveTheme, toggleMode } from "./lib/theme";
import { windowTitle } from "./lib/title";

type DialogName = "settings" | "about" | "tools" | null;

const DARK_QUERY = "(prefers-color-scheme: dark)";

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

  const open = useCallback(
    async (path: string) => {
      try {
        const file = await openImage(path);
        setImage(file);
        setError(null);
      } catch (e) {
        setError(isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : t("error.other", { path }));
      }
    },
    [t],
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
  }, [chooseFile, update]);

  // The bars appear on a mouse move and hide again after a pause.
  const onMouseMove = useCallback(() => {
    setRecentlyMoved(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setRecentlyMoved(false), CHROME_HIDE_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(hideTimer.current), []);

  useEffect(() => {
    // One file for now; the position in the folder comes with IV-03.
    const title = windowTitle(image?.name ?? null, 1, image ? 1 : 0, i18n.language);
    document.title = title;
    if (inTauri) void getCurrentWindow().setTitle(title);
  }, [image, i18n.language]);

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
        onLanguage={changeLanguage}
        onHover={setOverBar}
      />
      <main className="viewer">
        {image && (
          <img
            key={image.path}
            className="viewer__image"
            src={imageUrl(image)}
            alt={t("image.alt", { name: image.name })}
            draggable={false}
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
          onThemeChange={(theme) => update(() => ({ theme }))}
          onBackgroundChange={(background) => update(() => ({ background }))}
          onLanguage={changeLanguage}
          onAbout={() => setDialog("about")}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "about" && <AboutDialog onClose={() => setDialog(null)} />}
      {dialog === "tools" && <ToolsDialog onClose={() => setDialog(null)} />}
    </div>
  );
}
