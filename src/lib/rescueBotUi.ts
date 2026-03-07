import type {
  RescueBotRuntimeState,
  RescuePrimarySectionResult,
} from "@/lib/types";

export type RescuePrimaryAction = "set" | "activate" | "deactivate";
export type RescuePrimaryActionIcon = "play" | "pause";

export function getPrimaryRescueAction(
  runtimeState: RescueBotRuntimeState,
): RescuePrimaryAction {
  if (runtimeState === "active") {
    return "deactivate";
  }
  if (
    runtimeState === "unconfigured"
    || runtimeState === "configured_inactive"
    || runtimeState === "error"
  ) {
    return "activate";
  }
  return "activate";
}

export function shouldShowPrimaryRecovery(
  runtimeState: RescueBotRuntimeState,
): boolean {
  return runtimeState === "active";
}

export function isIconOnlyPrimaryRescueAction(
  runtimeState: RescueBotRuntimeState,
): boolean {
  switch (runtimeState) {
    case "unconfigured":
    case "configured_inactive":
    case "active":
    case "error":
      return true;
    default:
      return true;
  }
}

export function getPrimaryRescueActionIcon(
  runtimeState: RescueBotRuntimeState,
): RescuePrimaryActionIcon {
  return runtimeState === "active" ? "pause" : "play";
}

export function getIdleRescueProgress(
  runtimeState: RescueBotRuntimeState,
): number {
  switch (runtimeState) {
    case "active":
      return 1;
    case "configured_inactive":
      return 0.42;
    case "error":
      return 0.84;
    case "checking":
      return 0.58;
    case "unconfigured":
    default:
      return 0.16;
  }
}

export function buildStatusProgressLines(): string[] {
  return [
    "Refreshing helper state",
    "Reading rescue gateway status",
    "Updating recovery controls",
  ];
}

export function buildCheckProgressLines(): string[] {
  return [
    "Checking gateway configuration",
    "Checking models and credentials",
    "Checking tool execution policies",
    "Checking agent definitions",
    "Checking channel configuration",
    "Summarizing recovery plan",
  ];
}

export function buildFixProgressLines(
  sections: RescuePrimarySectionResult[],
): string[] {
  const sectionLines = sections
    .filter((section) => section.items.some((item) => item.autoFixable))
    .map((section) => `Fixing ${section.title} configuration`);
  if (sectionLines.length === 0) {
    sectionLines.push("Preparing recovery fix");
  }
  return [...sectionLines, "Rechecking recovery status", "Summarizing repair result"];
}
