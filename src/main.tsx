import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { Splash } from "./components/Splash";
import { guardPageZoom } from "./lib/pagezoom";
import { initSettings } from "./lib/settings";
import { SPLASH_DELAY_MS } from "./lib/splash";
import "./styles.css";

// IV-22: the WebView never zooms the whole page; Ctrl+wheel and Ctrl+plus/minus zoom the photo.
guardPageZoom(window);

const element = document.getElementById("root");
if (element) {
  const root = ReactDOM.createRoot(element);
  // The loading screen appears only if the start takes longer than 0.5 s (MV/STANDARD.md).
  const splash = window.setTimeout(() => root.render(<Splash />), SPLASH_DELAY_MS);
  void initSettings()
    .catch(() => undefined)
    .then(() => import("./i18n"))
    .then(() => {
      window.clearTimeout(splash);
      root.render(
        <React.StrictMode>
          <App />
        </React.StrictMode>,
      );
    });
}
