import { describe, expect, test } from "bun:test";

import {
  buildDataLoadLogLine,
  createDataLoadRequestId,
} from "../data-load-log";

describe("data load logging helpers", () => {
  test("creates stable prefixed request ids", () => {
    const first = createDataLoadRequestId("home.statusExtra");
    const second = createDataLoadRequestId("home.statusExtra");

    expect(first).toContain("home.statusExtra");
    expect(second).toContain("home.statusExtra");
    expect(first).not.toBe(second);
  });

  test("formats metrics log lines with required payload fields", () => {
    const line = buildDataLoadLogLine({
      requestId: "req-1",
      resource: "getInstanceConfigSnapshot",
      page: "home",
      instanceId: "ssh:lay2-dev",
      instanceToken: 42,
      source: "persisted",
      phase: "success",
      elapsedMs: 3,
      cacheHit: true,
    });

    expect(line).toContain("[metrics][data_load]");
    expect(line).toContain("\"resource\":\"getInstanceConfigSnapshot\"");
    expect(line).toContain("\"page\":\"home\"");
    expect(line).toContain("\"instanceId\":\"ssh:lay2-dev\"");
    expect(line).toContain("\"source\":\"persisted\"");
    expect(line).toContain("\"phase\":\"success\"");
    expect(line).toContain("\"cacheHit\":true");
  });
});
