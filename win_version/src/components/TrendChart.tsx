import { useMemo } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { axis as formatAxis, monthDay, percent } from "@/lib/format";
import type { DayStat, TokenMetricKey } from "@/types";

/**
 * Tokens over time, with the cache miss rate on a second axis.
 *
 * Misses and token volume share a scale only by accident, so plotting them
 * against the same axis would quietly suggest a relationship. Two axes keep
 * both readable, and the cache line is the one that explains a flat token
 * chart that still costs money.
 */
export function TrendChart({
  days,
  metric,
  color,
  label,
}: {
  days: DayStat[];
  metric: TokenMetricKey;
  color: string;
  label: string;
}) {
  const data = useMemo(
    () =>
      days.map((day) => ({
        date: day.date,
        value: day.tokens[metric],
        cost: day.cost,
        miss: day.cacheMiss,
        missRate: day.cacheMissRate,
        expected: day.cacheExpected,
      })),
    [days, metric],
  );

  // Recharts ignores non-finite values, which is exactly the right behaviour
  // for a window with no usage at all.
  const active = data.some((point) => point.value > 0 || point.miss > 0);

  return (
    <ResponsiveContainer width="100%" height={230}>
      <ComposedChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: 0 }}>
        <defs>
          <linearGradient id={`fill-${metric}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity={0.45} />
            <stop offset="100%" stopColor={color} stopOpacity={0.02} />
          </linearGradient>
        </defs>
        <CartesianGrid stroke="rgb(255 255 255 / 0.06)" vertical={false} />
        <XAxis
          dataKey="date"
          tickFormatter={monthDay}
          tick={{ fill: "#888F9F", fontSize: 10 }}
          axisLine={false}
          tickLine={false}
          minTickGap={24}
        />
        <YAxis
          yAxisId="tokens"
          tickFormatter={formatAxis}
          tick={{ fill: "#888F9F", fontSize: 10 }}
          axisLine={false}
          tickLine={false}
          width={44}
        />
        <YAxis
          yAxisId="miss"
          orientation="right"
          domain={[0, 100]}
          tickFormatter={(value: number) => `${Math.round(value)}%`}
          tick={{ fill: "#888F9F", fontSize: 10 }}
          axisLine={false}
          tickLine={false}
          width={32}
        />
        <Tooltip
          cursor={{ stroke: "rgb(255 255 255 / 0.18)" }}
          contentStyle={{
            background: "#0A0D14",
            border: "1px solid rgb(255 255 255 / 0.12)",
            borderRadius: 8,
            fontSize: 11,
          }}
          labelFormatter={(value) => String(value)}
          formatter={(value, name) => {
            if (name === "missRate") return [percent(Number(value)), "缓存缺口率"];
            if (name === "cost") return [`$${Number(value).toFixed(2)}`, "成本"];
            if (name === "miss") return [String(value), "缓存缺口"];
            if (name === "expected") return [String(value), "可缓存"];
            return [formatAxis(Number(value)), label];
          }}
        />
        {active && (
          <>
            <Area
              yAxisId="tokens"
              type="monotone"
              dataKey="value"
              stroke={color}
              strokeWidth={1.5}
              fill={`url(#fill-${metric})`}
              dot={false}
            />
            <Line
              yAxisId="miss"
              type="monotone"
              dataKey="missRate"
              stroke="var(--color-cache-miss)"
              strokeWidth={1.5}
              strokeDasharray="4 3"
              dot={false}
            />
          </>
        )}
      </ComposedChart>
    </ResponsiveContainer>
  );
}
