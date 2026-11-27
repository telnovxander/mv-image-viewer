import { describe, expect, it, vi } from "vitest";
import tauriConf from "../../src-tauri/tauri.conf.json";
import imageView from "../components/ImageView.tsx?raw";
import mainTsx from "../main.tsx?raw";
import { guardPageZoom, isPageZoomKey, requestZoom, ZOOM_EVENT } from "./pagezoom";

function fakeWindow() {
  const listeners: Record<string, { fn: (e: Event) => void; options: unknown }[]> = {};
  return {
    listeners,
    addEventListener: (type: string, fn: (e: Event) => void, options?: unknown) => {
      (listeners[type] ??= []).push({ fn, options });
    },
    removeEventListener: (type: string, fn: (e: Event) => void) => {
      listeners[type] = (listeners[type] ?? []).filter((l) => l.fn !== fn);
    },
  };
}

describe("the page zoom guard (IV-22)", () => {
  it("knows the browser's zoom keys", () => {
    for (const key of ["+", "=", "-", "_", "0"]) expect(isPageZoomKey({ ctrlKey: true, metaKey: false, key })).toBe(true);
    expect(isPageZoomKey({ ctrlKey: false, metaKey: false, key: "+" })).toBe(false);
    expect(isPageZoomKey({ ctrlKey: true, metaKey: false, key: "o" })).toBe(false);
  });

  it("prevents the default of Ctrl+wheel and the zoom keys, in the capture phase, with a non-passive wheel listener", () => {
    const win = fakeWindow();
    const remove = guardPageZoom(win as unknown as Window);
    const wheel = win.listeners.wheel?.[0];
    expect(wheel?.options).toEqual({ capture: true, passive: false });
    const ctrlWheel = { ctrlKey: true, metaKey: false, preventDefault: vi.fn() };
    wheel?.fn(ctrlWheel as unknown as Event);
    expect(ctrlWheel.preventDefault).toHaveBeenCalled();
    const plainWheel = { ctrlKey: false, metaKey: false, preventDefault: vi.fn() };
    wheel?.fn(plainWheel as unknown as Event);
    expect(plainWheel.preventDefault).not.toHaveBeenCalled();
    const key = { ctrlKey: true, metaKey: false, key: "+", preventDefault: vi.fn() };
    win.listeners.keydown?.[0]?.fn(key as unknown as Event);
    expect(key.preventDefault).toHaveBeenCalled();
    remove();
    expect(win.listeners.wheel).toHaveLength(0);
    expect(win.listeners.keydown).toHaveLength(0);
  });

  it("is installed for the whole window before the interface starts", () => {
    expect(mainTsx).toMatch(/guardPageZoom\(window\)/);
  });

  it("lets the WebView deliver Ctrl+wheel and the zoom keys to the page (WebView2 IsZoomControlEnabled)", () => {
    expect(tauriConf.app.windows[0]?.zoomHotkeysEnabled).toBe(true);
  });

  it("the image view takes Ctrl+wheel from the whole window and the zoom buttons' requests", () => {
    expect(imageView).toMatch(/window\.addEventListener\("wheel", onCtrlWheel, \{ capture: true, passive: false \}\)/);
    expect(imageView).toMatch(/window\.addEventListener\(ZOOM_EVENT, onZoomRequest\)/);
  });

  it("the zoom buttons send one step in or out", () => {
    const target = new EventTarget();
    const got: number[] = [];
    target.addEventListener(ZOOM_EVENT, (event) => got.push((event as CustomEvent<number>).detail));
    requestZoom(1, target as unknown as Window);
    requestZoom(-1, target as unknown as Window);
    expect(got).toEqual([1, -1]);
  });
});
