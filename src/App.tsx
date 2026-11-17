import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { imageUrl, inTauri, initialPath, isBackendError, openImage, type ImageFile } from "./lib/backend";
import { WEB_EXTENSIONS } from "./lib/formats";
import { windowTitle } from "./lib/title";

export function App() {
  const { t, i18n } = useTranslation();
  const [image, setImage] = useState<ImageFile | null>(null);
  const [error, setError] = useState<string | null>(null);

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
      if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void chooseFile();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [chooseFile]);

  useEffect(() => {
    // One file for now; the position in the folder comes with IV-03.
    const title = windowTitle(image?.name ?? null, 1, image ? 1 : 0, i18n.language);
    document.title = title;
    if (inTauri) void getCurrentWindow().setTitle(title);
  }, [image, i18n.language]);

  return (
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
  );
}
