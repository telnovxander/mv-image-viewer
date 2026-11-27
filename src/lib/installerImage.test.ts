import { describe, expect, it } from "vitest";
import tauriConf from "../../src-tauri/tauri.conf.json";

// MV-15: Lumi on the Welcome and Finish pages of the installer, a copy of MV projects/MV/brand/installer/.
describe("the installer picture (MV-15)", () => {
  it("is the sidebar of the NSIS Welcome and Finish pages", () => {
    expect(tauriConf.bundle.windows.nsis.sidebarImage).toBe("../assets/brand/installer/spiegelmond-installer-sidebar.bmp");
  });
});
