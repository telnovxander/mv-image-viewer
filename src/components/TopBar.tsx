import { CircleHelp, FolderOpen, Settings as SettingsIcon, Wrench } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Mode } from "../lib/settings";
import { LanguageSelect } from "./LanguageSelect";
import { ModeIcon } from "./ModeIcon";

interface TopBarProps {
  mode: Mode;
  visible: boolean;
  onOpen: () => void;
  onToggleMode: () => void;
  onTools: () => void;
  onSettings: () => void;
  onHelp: () => void;
  onLanguage: (code: string) => void;
  onHover: (inside: boolean) => void;
}

/** The bar over the image: it appears when the mouse moves and hides again after a moment. */
export function TopBar({ mode, visible, onOpen, onToggleMode, onTools, onSettings, onHelp, onLanguage, onHover }: TopBarProps) {
  const { t } = useTranslation();
  const modeName = mode === "sichel" ? "Sichel" : "Vollmond";
  return (
    <header
      className={visible ? "topbar topbar--visible" : "topbar"}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      onFocus={() => onHover(true)}
      onBlur={() => onHover(false)}
    >
      <button type="button" className="icon-button" onClick={onOpen} title={t("open.button", { shortcut: "Ctrl+O" })} aria-label={t("open.button", { shortcut: "Ctrl+O" })}>
        <FolderOpen size={16} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="topbar__mode"
        onClick={onToggleMode}
        title={t("mode.toggle", { shortcut: "Ctrl+M" })}
        aria-pressed={mode === "vollmond"}
      >
        <ModeIcon mode={mode} />
        <span>{modeName}</span>
      </button>
      {mode === "vollmond" && (
        <button type="button" className="icon-button icon-button--accent" onClick={onTools} title={t("tools.title")} aria-label={t("tools.title")}>
          <Wrench size={16} aria-hidden="true" />
        </button>
      )}
      <div className="topbar__spacer" />
      <LanguageSelect onChange={onLanguage} />
      <button type="button" className="icon-button" onClick={onHelp} title={t("help.button", { shortcut: "F1" })} aria-label={t("help.button", { shortcut: "F1" })}>
        <CircleHelp size={16} aria-hidden="true" />
      </button>
      <button type="button" className="icon-button" onClick={onSettings} title={t("settings.title")} aria-label={t("settings.title")}>
        <SettingsIcon size={16} aria-hidden="true" />
      </button>
    </header>
  );
}
