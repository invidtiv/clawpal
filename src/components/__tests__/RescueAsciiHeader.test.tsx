import { describe, expect, test } from "bun:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { RescueAsciiHeader } from "../RescueAsciiHeader";

describe("RescueAsciiHeader", () => {
  test("renders a deterministic wide console pixel bot with uparrow eyes only when active", () => {
    const activeHtml = renderToStaticMarkup(
      React.createElement(RescueAsciiHeader, {
        state: "active",
        title: "Helper is enabled",
        progress: 0.5,
      }),
    );
    const pausedHtml = renderToStaticMarkup(
      React.createElement(RescueAsciiHeader, {
        state: "configured_inactive",
        title: "Helper is paused",
        progress: 0.25,
      }),
    );

    expect(activeHtml).toContain("data-led-bot=\"wide-console\"");
    expect(activeHtml).toContain("data-bot-cell=\"body\"");
    expect(activeHtml.match(/data-bot-cell=\"progress-fill\"/g)?.length).toBe(6);
    expect(activeHtml.match(/data-bot-cell=\"progress-empty\"/g)?.length).toBe(6);
    expect(activeHtml.match(/data-bot-cell=\"eye\"/g)?.length).toBe(2);
    expect(activeHtml.match(/data-bot-cell=\"mouth\"/g)?.length ?? 0).toBe(0);
    expect(activeHtml.match(/data-bot-eye-expression=\"uparrow\"/g)?.length).toBe(2);
    expect(activeHtml).toContain("h-[14px] w-[20px] sm:h-[16px] sm:w-[22px]");
    expect(activeHtml).toContain("translate-y-[2px]");
    expect(activeHtml).toContain("origin-right rotate-45");
    expect(activeHtml).toContain("origin-left -rotate-45");
    expect(activeHtml.match(/data-bot-cell=\"leg\"/g)?.length).toBe(8);
    expect(activeHtml).toContain("h-[10px] w-[10px]");
    expect(activeHtml).toContain("sm:h-[12px] sm:w-[12px]");
    expect(pausedHtml.match(/data-bot-cell=\"progress-fill\"/g)?.length).toBe(3);
    expect(pausedHtml.match(/data-bot-cell=\"progress-empty\"/g)?.length).toBe(9);
    expect(pausedHtml.match(/data-bot-cell=\"mouth\"/g)?.length ?? 0).toBe(0);
    expect(pausedHtml.match(/data-bot-eye-expression=\"idle\"/g)?.length).toBe(2);
    expect(activeHtml).not.toContain("<pre");
    expect(activeHtml).not.toContain("emerald");
    expect(activeHtml).not.toContain("sky");
    expect(activeHtml).toContain("text-center");
  });
});
