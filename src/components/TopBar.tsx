import { FolderOpen, Pause, Play, Settings as SettingsIcon, Wrench, ZoomIn, ZoomOut } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Mode } from "../lib/settings";
import { HelpMenu, type HelpItem } from "./HelpMenu";
import { LanguageSelect } from "./LanguageSelect";
import { ModeIcon } from "./ModeIcon";

interface TopBarProps {
  mode: Mode;
  visible: boolean;
  onOpen: () => void;
  onToggleMode: () => void;
  onTools: () => void;
  onSettings: () => void;
  /** An item of the "?" menu (IV-20). */
  onHelp: (item: HelpItem) => void;
  onHelpMenu: (open: boolean) => void;
  /** The slideshow button (IV-20): start · stop, as Space. */
  slideshow: boolean;
  canSlideshow: boolean;
  onSlideshow: () => void;
  /** The magnifiers (IV-22): one zoom step out (-1) or in (1), as Ctrl+minus / plus. */
  canZoom: boolean;
  onZoom: (direction: 1 | -1) => void;
  onLanguage: (code: string) => void;
  /** The mouse is over the bar. */
  onHover: (inside: boolean) => void;
  /** A button of the bar has the keyboard focus (focus left by a mouse click does not count). */
  onKeyboardFocus: (focused: boolean) => void;
}

/** The bar over the image: it appears when the mouse moves and hides again after a moment. */
export function TopBar(props: TopBarProps) {
  const { mode, visible, onOpen, onToggleMode, onTools, onSettings, onHelp, onHelpMenu, slideshow, canSlideshow, onSlideshow, canZoom, onZoom, onLanguage, onHover, onKeyboardFocus } = props;
  const { t } = useTranslation();
  const modeName = mode === "sichel" ? "Sichel" : "Vollmond";
  const slideLabel = t(slideshow ? "slideshow.stop" : "slideshow.start", { shortcut: t("keys.Space") });
  return (
    <header
      className={visible ? "topbar topbar--visible" : "topbar"}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      onFocus={(event) => onKeyboardFocus(event.target.matches(":focus-visible"))}
      onBlur={() => onKeyboardFocus(false)}
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
      <button type="button" className="icon-button" onClick={onSlideshow} disabled={!canSlideshow} aria-pressed={slideshow} title={slideLabel} aria-label={slideLabel}>
        {slideshow ? <Pause size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}
      </button>
      <button type="button" className="icon-button" onClick={() => onZoom(-1)} disabled={!canZoom} title={t("zoom.out", { shortcut: "Ctrl+−" })} aria-label={t("zoom.out", { shortcut: "Ctrl+−" })}>
        <ZoomOut size={16} aria-hidden="true" />
      </button>
      <button type="button" className="icon-button" onClick={() => onZoom(1)} disabled={!canZoom} title={t("zoom.in", { shortcut: "Ctrl++" })} aria-label={t("zoom.in", { shortcut: "Ctrl++" })}>
        <ZoomIn size={16} aria-hidden="true" />
      </button>
      <div className="topbar__spacer" />
      <LanguageSelect onChange={onLanguage} />
      <HelpMenu onChoose={onHelp} onOpenChange={onHelpMenu} />
      <button type="button" className="icon-button" onClick={onSettings} title={t("settings.title")} aria-label={t("settings.title")}>
        <SettingsIcon size={16} aria-hidden="true" />
      </button>
    </header>
  );
}
