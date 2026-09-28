import { useMemo } from "react";
import { Cell, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";

import { currency, percent, shareOf, tokens as formatTokens } from "@/lib/format";
import type { ProviderStat } from "@/types";

const PALETTE = [
  "var(--color-chart-1)",
  "var(--color-chart-2)",
  "var(--color-chart-3)",
  "var(--color-chart-4)",
  "var(--color-chart-5)",
];
const MUTED = "var(--color-muted)";

/** Beyond this the slices stop being distinguishable, so the tail is folded. */
const TOP_SLICES = 6;

export function ProviderDonut({ providers }: { providers: ProviderStat[] }) {
  const { slices, total } = useMemo(() => {
    const sorted = [...providers].sort((a, b) => b.tokens.total - a.tokens.total);
    const head = sorted.slice(0, TOP_SLICES);
    const tail = sorted.slice(TOP_SLICES);
    const rows = head.map((row) => ({
      name: row.name,
      value: row.tokens.total,
      cost: row.cost,
      messageCount: row.messageCount,
    }));
    if (tail.length > 0) {
      rows.push({
        name: "其他",
        value: tail.reduce((sum, row) => sum + row.tokens.total, 0),
        cost: tail.reduce((sum, row) => sum + row.cost, 0),
        messageCount: tail.reduce((sum, row) => sum + row.messageCount, 0),
      });
    }
    return {
      slices: rows,
      total: sorted.reduce((sum, row) => sum + row.tokens.total, 0),
    };
  }, [providers]);

  if (total === 0) {
    return <p className="py-8 text-center text-[11px] text-muted">窗口内没有服务商用量</p>;
  }

  return (
    <div className="flex items-center gap-3">
      <ResponsiveContainer width="50%" height={170}>
        <PieChart>
          <Pie
            data={slices}
            dataKey="value"
            nameKey="name"
            innerRadius={48}
            outerRadius={72}
            paddingAngle={1.5}
            stroke="none"
          >
            {slices.map((slice, index) => (
              <Cell
                key={slice.name}
                fill={index < TOP_SLICES ? PALETTE[index % PALETTE.length] : MUTED}
              />
            ))}
          </Pie>
          <Tooltip
            contentStyle={{
              background: "#0A0D14",
              border: "1px solid rgb(255 255 255 / 0.12)",
              borderRadius: 8,
              fontSize: 11,
            }}
            formatter={(value, name) => [
              `${formatTokens(Number(value))} · ${shareOf(Number(value), total)}`,
              String(name),
            ]}
          />
        </PieChart>
      </ResponsiveContainer>
      <ul className="flex min-w-0 flex-1 flex-col gap-1">
        {slices.map((slice, index) => (
          <li key={slice.name} className="flex items-center gap-1.5 text-[10px]">
            <span
              className="size-2 shrink-0 rounded-[2px]"
              style={{ backgroundColor: index < TOP_SLICES ? PALETTE[index % PALETTE.length] : MUTED }}
            />
            <span className="min-w-0 flex-1 truncate" title={slice.name}>
              {slice.name}
            </span>
            <span className="tnum shrink-0 text-muted">{percent((slice.value / total) * 100)}</span>
          </li>
        ))}
        <li className="tnum mt-1 border-t border-white/8 pt-1 text-[10px] text-muted">
          合计 {formatTokens(total)} · {currency(slices.reduce((sum, slice) => sum + slice.cost, 0))}
        </li>
      </ul>
    </div>
  );
}
