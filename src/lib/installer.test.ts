import { describe, expect, it } from "vitest";
import installer from "../../src-tauri/nsis/installer.nsi?raw";
import tauriConf from "../../src-tauri/tauri.conf.json";
import english from "../../src-tauri/nsis/languages/English.nsh?raw";
import german from "../../src-tauri/nsis/languages/German.nsh?raw";
import russian from "../../src-tauri/nsis/languages/Russian.nsh?raw";

// MV-14: the same installer code and texts in Halbmond and Spiegelmond (this file is identical in
// both repositories): an installed copy is named on the Welcome page, a stale entry counts as not
// installed, a silent downgrade is refused with an exit code, and the texts are human ones.
describe("an installed copy (MV-14)", () => {
  it("is named on the Welcome page with both versions", () => {
    expect(installer).toMatch(/MUI_PAGE_CUSTOMFUNCTION_PRE MvWelcomePre/);
    expect(installer).toMatch(/MUI_WELCOMEPAGE_TEXT "\$\(\w+WelcomeText\)\$MvWelcomeNote"/);
    expect(installer).toMatch(/Function MvWelcomePre[\s\S]*?mvInstalledSame[\s\S]*?mvInstalledOlder[\s\S]*?FunctionEnd/);
  });

  it("a stale entry (folder deleted by hand) is not an installation", () => {
    const checks = installer.match(/\$\{IfNot\} \$\{FileExists\} "\$4\\uninstall\.exe"/g) ?? [];
    expect(checks).toHaveLength(2);
  });

  it("a silent downgrade is refused with a non-zero exit code", () => {
    expect(tauriConf.bundle.windows.allowDowngrades).toBe(false);
    expect(installer).toMatch(/SetErrorLevel 2[^\n]*\n\s+Abort/);
  });

  it("every language has the texts, with the versions, and no machine-translated leftovers", () => {
    for (const [name, file] of [["English", english], ["German", german], ["Russian", russian]] as const) {
      for (const key of ["mvInstalledSame", "mvInstalledOlder", "appRunning", "appRunningOkKill", "failedToKillApp", "silentDowngrades", "unableToUninstall"]) {
        expect(file, `${name} ${key}`).toMatch(new RegExp(`^LangString ${key} `, "m"));
      }
      expect(file, name).toMatch(/LangString mvInstalledSame [^\n]*\$MvInstalledVersion/);
      expect(file, name).toMatch(/LangString mvInstalledOlder [^\n]*\$MvInstalledVersion[^\n]*\$\{VERSION\}/);
      expect(file, name).not.toMatch(/попробуйте еще раз|Failed to kill|Click OK to kill/);
    }
  });
});
