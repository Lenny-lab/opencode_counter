import { useMemo } from "react";

import { axis } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { HeatCell } from "@/types";

const BLOCKS = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22] as const;

/** Opacity ramp for the five intensity steps, plus the empty cell. */
const LEVELS = [
  "bg-white/[0.07]",
  "bg-token-total/20",
  "bg-token-total/35",
  "bg-token-total/50",
  "bg-token-total/70",
  "bg-token-total/90",
];

/**
 * Intensity bucket for a cell, using the same curve as the original dashboard.
 *
 * A square-root ratio is what makes the grid readable: raw token counts span
 * several orders of magnitude, so a linear ramp would leave everything but the
 * single busiest block flat.
 */
function level(value: number, max: number): number {
  if (value <= 0 || max <= 0) return 0;
  const ratio = Math.sqrt(value / max);
  return Math.min(5, Math.max(1, Math.ceil(ratio * 5)));
}

/** Metric the cells are coloured by, fixed to the selected dashboard metric. */
type Metric = "total" | "input" | "output" | "reasoning" | "cacheRead" | "cacheWrite" | "messageCount";

export function Heatmap({ cells, metric }: { cells: HeatCell[]; metric: Metric }) {
  const { grid, days, max } = useMemo(() => {
    const byDay = new Map<string, Map<number, HeatCell>>();
    for (const cell of cells) {
      let row = byDay.get(cell.date);
      if (!row) {
        row = new Map();
        byDay.set(cell.date, row);
      }
      row.set(cell.block, cell);
    }
    const dates = [...byDay.keys()].sort();
    let highest = 0;
    for (const cell of cells) highest = Math.max(highest, valueOf(cell, metric));
    return { grid: byDay, days: dates, max: highest };
  }, [cells, metric]);

  if (days.length === 0) {
    return <p className="py-8 text-center text-[11px] text-muted">窗口内没有活动记录</p>;
  }

  return (
    <div className="flex gap-1.5 overflow-x-auto pb-1">
      <div className="grid shrink-0 grid-rows-[repeat(12,14px)_auto] gap-[3px] pr-1 text-right">
        {BLOCKS.map((block) => (
          <span key={block} className="tnum text-[9px] leading-[14px] text-muted">
            {block % 4 === 0 ? String(block).padStart(2, "0") : ""}
          </span>
        ))}
        <span />
      </div>
      <div
        className="grid gap-[3px]"
        style={{ gridTemplateColumns: `repeat(${days.length}, 14px)` }}
      >
        {BLOCKS.flatMap((block) =>
          days.map((day) => {
            const cell = grid.get(day)?.get(block);
            const value = cell ? valueOf(cell, metric) : 0;
            const step = level(value, max);
            return (
              <div
                key={`${day}#${block}`}
                className={cn("size-[14px] rounded-[3px]", LEVELS[step])}
                title={
                  cell
                    ? `${day} ${String(block).padStart(2, "0")}:00 · ${axis(value)} · ${cell.messageCount} 条消息`
                    : `${day} ${String(block).padStart(2, "0")}:00 · 无活动`
                }
              />
            );
          }),
        )}
        <div />
        <div className="flex gap-[3px]" style={{ gridColumn: `1 / span ${days.length}` }}>
          {days.map((day, index) => (
            <span
              key={day}
              className="tnum w-[14px] text-center text-[9px] text-muted"
            >
              {/* One label per week keeps the axis readable at any width. */}
              {index % 7 === 0 ? day.slice(5) : ""}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function valueOf(cell: HeatCell, metric: Metric): number {
  return metric === "messageCount" ? cell.messageCount : cell.tokens[metric];
}
