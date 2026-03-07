import type { RescueBotRuntimeState } from "@/lib/types";
import { cn } from "@/lib/utils";

const BOT_MATRIX = [
  ".....cc........cc.....",
  "....cccc......cccc....",
  "......bbbbbbbbbb......",
  "....bbbbbbbbbbbbbb....",
  "...bbb..........bbb...",
  "..bbbb..e....e..bbbb..",
  "..bbbb..........bbbb..",
  "..bbbb..........bbbb..",
  "...bbb..........bbb...",
  "....bbbbbbbbbbbbbb....",
  ".....tttttttttttt.....",
  ".........ll..ll.......",
  ".........ll..ll.......",
] as const;

const GRID_WIDTH = BOT_MATRIX[0].length;
const PROGRESS_SLOTS = 12;

type BotCellToken = "." | "b" | "c" | "e" | "l" | "t";

const bodyToneByState: Record<RescueBotRuntimeState, string> = {
  unconfigured: "bg-neutral-700",
  configured_inactive: "bg-neutral-900",
  active: "bg-neutral-900",
  checking: "bg-neutral-900",
  error: "bg-neutral-900",
};

const eyeToneByState: Record<RescueBotRuntimeState, string> = {
  unconfigured: "bg-neutral-500",
  configured_inactive: "bg-neutral-500",
  active: "bg-neutral-900",
  checking: "bg-neutral-700",
  error: "bg-red-700",
};

const progressFillToneByState: Record<RescueBotRuntimeState, string> = {
  unconfigured: "bg-neutral-500",
  configured_inactive: "bg-neutral-700",
  active: "bg-neutral-900",
  checking: "bg-neutral-800",
  error: "bg-neutral-900",
};

interface RescueAsciiHeaderProps {
  state: RescueBotRuntimeState;
  title: string;
  progress?: number;
  animateProgress?: boolean;
}

function clampProgress(progress?: number): number {
  if (typeof progress !== "number" || Number.isNaN(progress)) {
    return 0;
  }
  return Math.max(0, Math.min(1, progress));
}

function cellLabel(token: BotCellToken, progressIndex: number, filledSlots: number) {
  switch (token) {
    case "b":
    case "c":
      return "body";
    case "e":
      return "eye";
    case "l":
      return "leg";
    case "t":
      return progressIndex < filledSlots ? "progress-fill" : "progress-empty";
    default:
      return "empty";
  }
}

export function RescueAsciiHeader({
  state,
  title,
  progress,
  animateProgress = false,
}: RescueAsciiHeaderProps) {
  const clampedProgress = clampProgress(progress);
  const filledSlots = Math.round(clampedProgress * PROGRESS_SLOTS);
  let progressIndex = 0;

  return (
    <div className="min-w-0 text-center">
      <div
        role="img"
        aria-label={title}
        title={title}
        data-led-bot="wide-console"
        className="inline-grid gap-[1px] justify-center"
        style={{ gridTemplateColumns: `repeat(${GRID_WIDTH}, minmax(0, 1fr))` }}
      >
        {BOT_MATRIX.flatMap((row, rowIndex) =>
          row.split("").map((token, columnIndex) => {
            const typedToken = token as BotCellToken;
            const label = cellLabel(typedToken, progressIndex, filledSlots);
            const isFilledProgress = typedToken === "t" && progressIndex < filledSlots;
            const isProgress = typedToken === "t";
            if (typedToken === "t") {
              progressIndex += 1;
            }

            return (
              <span
                key={`${rowIndex}-${columnIndex}`}
                data-bot-cell={label}
                aria-hidden="true"
                className={cn(
                  "inline-flex h-[10px] w-[10px] items-center justify-center sm:h-[12px] sm:w-[12px]",
                  typedToken === "." && "opacity-0",
                  (typedToken === "b" || typedToken === "c") && bodyToneByState[state],
                  typedToken === "l" && "bg-stone-400",
                  isProgress && "bg-neutral-300",
                  isFilledProgress &&
                    cn(
                      progressFillToneByState[state],
                      animateProgress && "animate-pulse transition-colors duration-300",
                    ),
                )}
              >
                {typedToken === "e" ? (
                  <span
                    className={cn(
                      "h-[5px] w-[5px] rounded-full sm:h-[6px] sm:w-[6px]",
                      eyeToneByState[state],
                      state === "checking" && "animate-pulse",
                    )}
                  />
                ) : null}
              </span>
            );
          }),
        )}
      </div>
    </div>
  );
}
