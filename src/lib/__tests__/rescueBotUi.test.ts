import { describe, expect, test } from "bun:test";

import type { RescuePrimarySectionResult } from "@/lib/types";
import {
  buildCheckProgressLines,
  buildFixProgressLines,
  buildStatusProgressLines,
  getPrimaryRescueAction,
  getPrimaryRescueActionIcon,
  getIdleRescueProgress,
  isIconOnlyPrimaryRescueAction,
  shouldShowPrimaryRecovery,
} from "@/lib/rescueBotUi";

describe("rescueBotUi", () => {
  test("maps rescue runtime state to a single enable or disable action", () => {
    expect(getPrimaryRescueAction("unconfigured")).toBe("activate");
    expect(getPrimaryRescueAction("configured_inactive")).toBe("activate");
    expect(getPrimaryRescueAction("active")).toBe("deactivate");
    expect(getPrimaryRescueAction("error")).toBe("activate");
  });

  test("only shows primary recovery when rescue bot is active", () => {
    expect(shouldShowPrimaryRecovery("unconfigured")).toBe(false);
    expect(shouldShowPrimaryRecovery("configured_inactive")).toBe(false);
    expect(shouldShowPrimaryRecovery("error")).toBe(false);
    expect(shouldShowPrimaryRecovery("active")).toBe(true);
  });

  test("uses icon-only primary action for all rescue states", () => {
    expect(isIconOnlyPrimaryRescueAction("unconfigured")).toBe(true);
    expect(isIconOnlyPrimaryRescueAction("configured_inactive")).toBe(true);
    expect(isIconOnlyPrimaryRescueAction("error")).toBe(true);
    expect(isIconOnlyPrimaryRescueAction("active")).toBe(true);
  });

  test("maps rescue runtime state to play or pause icons", () => {
    expect(getPrimaryRescueActionIcon("unconfigured")).toBe("play");
    expect(getPrimaryRescueActionIcon("configured_inactive")).toBe("play");
    expect(getPrimaryRescueActionIcon("error")).toBe("play");
    expect(getPrimaryRescueActionIcon("active")).toBe("pause");
  });

  test("exposes a stable idle progress baseline for each rescue state", () => {
    expect(getIdleRescueProgress("unconfigured")).toBe(0.16);
    expect(getIdleRescueProgress("configured_inactive")).toBe(0.42);
    expect(getIdleRescueProgress("checking")).toBe(0.58);
    expect(getIdleRescueProgress("error")).toBe(0.84);
    expect(getIdleRescueProgress("active")).toBe(1);
  });

  test("builds a fixed single-line status progress sequence", () => {
    expect(buildStatusProgressLines()).toEqual([
      "Refreshing helper state",
      "Reading rescue gateway status",
      "Updating recovery controls",
    ]);
  });

  test("builds a fixed single-line check progress sequence", () => {
    expect(buildCheckProgressLines()).toEqual([
      "Checking gateway configuration",
      "Checking models and credentials",
      "Checking tool execution policies",
      "Checking agent definitions",
      "Checking channel configuration",
      "Summarizing recovery plan",
    ]);
  });

  test("builds fix progress lines from fixable sections and always rechecks", () => {
    const sections: RescuePrimarySectionResult[] = [
      {
        key: "gateway",
        title: "Gateway",
        status: "broken",
        summary: "Gateway needs attention",
        docsUrl: "https://docs.openclaw.ai/gateway/security/index",
        items: [
          {
            id: "primary.gateway.unhealthy",
            label: "Primary gateway is not healthy",
            status: "error",
            detail: "restart required",
            autoFixable: false,
            issueId: "primary.gateway.unhealthy",
          },
        ],
      },
      {
        key: "agents",
        title: "Agents",
        status: "degraded",
        summary: "Agent defaults need setup",
        docsUrl: "https://docs.openclaw.ai/agents",
        items: [
          {
            id: "field.agents",
            label: "Missing agent defaults",
            status: "warn",
            detail: "can auto-fix",
            autoFixable: true,
            issueId: "field.agents",
          },
        ],
      },
    ];

    expect(buildFixProgressLines(sections)).toEqual([
      "Fixing Agents configuration",
      "Rechecking recovery status",
      "Summarizing repair result",
    ]);
  });
});
