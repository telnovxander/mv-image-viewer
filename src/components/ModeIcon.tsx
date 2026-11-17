import sichel from "../../assets/brand/modes/sichel.svg?raw";
import vollmond from "../../assets/brand/modes/vollmond.svg?raw";
import type { Mode } from "../lib/settings";

/** The Sichel / Vollmond mode icon; both drawings are stacked and cross-fade (~300 ms). */
export function ModeIcon({ mode }: { mode: Mode }) {
  return (
    <span className={`mode-icon mode-icon--${mode}`} aria-hidden="true">
      <span className="mode-icon__sichel" dangerouslySetInnerHTML={{ __html: sichel }} />
      <span className="mode-icon__vollmond" dangerouslySetInnerHTML={{ __html: vollmond }} />
    </span>
  );
}
