import { describe, expect, test } from "bun:test";

import { resolveDoctorOverlayVisibility } from "../doctor-overlay";

describe("resolveDoctorOverlayVisibility", () => {
  test("hides the guidance card when the assistant pill is hidden", () => {
    expect(
      resolveDoctorOverlayVisibility({
        showDoctorUi: false,
        guidanceOpen: true,
        hasGuidance: true,
      }),
    ).toEqual({
      showQuickDiagnose: false,
      showGuidanceCard: false,
      showAssistantPill: false,
      showGuidanceOverlay: false,
    });
  });

  test("keeps quick diagnose available without rendering the assistant pill", () => {
    expect(
      resolveDoctorOverlayVisibility({
        showDoctorUi: true,
        guidanceOpen: false,
        hasGuidance: false,
      }),
    ).toEqual({
      showQuickDiagnose: true,
      showGuidanceCard: false,
      showAssistantPill: false,
      showGuidanceOverlay: false,
    });
  });
});
