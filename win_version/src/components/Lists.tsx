import { useMemo } from "react";

import { currency, number, percent, providerLabel, shareOf, tokens as formatTokens } from "@/lib/format";
import type { ModelStat, ProjectStat, ToolStat } from "@/types";
import { Bar, Empty } from "@/components/ui/primitives";

const CHART_COLOURS = [
  "var(--color-chart-1)",
  "var(--color-chart-2)",
  "var(--color-chart-3)",
  "var(--color-chart-4)",
  "var(--color-chart-5)",
];

/**
 * Models are merged by name, so one row can carry several providers. The row
 * is sized by the selected metric, not by cost: with free models cost is often
 * zero across the board and would make the chart flat.
 */
export function ModelList({ models, metric }: { models: ModelStat[]; metric: keyof ModelStat["tokens"] }) {
  const rows = useMemo(
    () => [...models].sort((a, b) => b.tokens[metric] - a.tokens[metric]).slice(0, 10),
    [models, metric],
  );
  const total = models.reduce((sum, row) => sum + row.tokens[metric], 0);
  if (rows.length === 0 || total === 0) return <Empty>窗口内没有模型用量</Empty>;
  return (
    <ul className="flex flex-col gap-1.5">
      {rows.map((row, index) => (
        <li key={row.name}>
          <div className="flex items-baseline justify-between gap-2">
            <span className="truncate text-[11px]" title={row.name}>
              {row.name}
            </span>
            <span className="tnum shrink-0 text-[10px] text-muted">
              {formatTokens(row.tokens[metric])} · {shareOf(row.tokens[metric], total)}
            </span>
          </div>
          <div className="mt-0.5">
            <Bar ratio={row.tokens[metric] / Math.max(1, rows[0].tokens[metric])} color={CHART_COLOURS[index % CHART_COLOURS.length]} />
          </div>
          <div className="mt-0.5 flex items-center justify-between text-[10px] text-muted">
            <span className="truncate">{providerLabel(row.providers)}</span>
            <span className="tnum shrink-0">
              {number(row.messageCount)} 条 · {currency(row.cost)}
            </span>
          </div>
        </li>
      ))}
    </ul>
  );
}

export function ProjectList({ projects }: { projects: ProjectStat[] }) {
  const rows = useMemo(
    () => [...projects].sort((a, b) => b.tokens.total - a.tokens.total),
    [projects],
  );
  if (rows.length === 0) return <Empty>窗口内没有项目</Empty>;
  const peak = Math.max(1, rows[0].tokens.total);
  return (
    <ul className="flex flex-col gap-1.5">
      {rows.map((row) => (
        <li key={row.id}>
          <div className="flex items-baseline justify-between gap-2">
            <span className="truncate text-[11px]" title={row.path}>
              {row.name}
            </span>
            <span className="tnum shrink-0 text-[10px] text-muted">
              {formatTokens(row.tokens.total)} · {currency(row.cost)}
            </span>
          </div>
          <div className="mt-0.5">
            <Bar ratio={row.tokens.total / peak} color={CHART_COLOURS[4]} />
          </div>
          <div className="mt-0.5 flex items-center justify-between text-[10px] text-muted">
            <span className="truncate">{row.latestSessionTitle ?? "—"}</span>
            <span className="tnum shrink-0">
              {number(row.sessionCount)} 会话 · {number(row.messageCount)} 消息
            </span>
          </div>
        </li>
      ))}
    </ul>
  );
}

export function ToolList({ tools }: { tools: ToolStat[] }) {
  const rows = tools.slice(0, 12);
  if (rows.length === 0) return <Empty>窗口内没有工具调用</Empty>;
  const peak = Math.max(1, rows[0].count);
  return (
    <ul className="flex flex-col gap-1">
      {rows.map((tool) => (
        <li key={tool.name} className="flex items-center gap-2">
          <span className="w-20 shrink-0 truncate text-[10px] text-muted" title={tool.name}>
            {tool.name}
          </span>
          <Bar ratio={tool.count / peak} color={CHART_COLOURS[1]} />
          <span className="tnum w-16 shrink-0 text-right text-[10px]">
            {number(tool.count)}
            <span className="ml-1 text-muted">{percent(tool.percentage)}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
