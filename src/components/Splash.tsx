import splash from "../../assets/brand/splash-dark.jpg";

/** Loading screen, shown only when the start takes longer than 0.5 s (see lib/splash.ts).
 * The name and the version sit in the empty sky top left, away from the moon and its reflection. */
export function Splash() {
  return (
    <div className="splash" role="status">
      <img className="splash__image" src={splash} alt="" />
      <div className="splash__title">
        <span className="splash__name">Spiegelmond</span>
        <span className="splash__version">{__APP_VERSION__}</span>
      </div>
    </div>
  );
}
