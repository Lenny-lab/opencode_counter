import { useMemo, useState } from "react";
import { Tabs } from "radix-ui";
import {
  Activity,
  Boxes,
  FolderGit2,
  Gauge,
  RefreshCw,
  ExternalLink,
  X,
} from "lucide-react";

import { hidePopover, openDashboard } from "@/lib/ipc";
import { useSnapshot } from "@/lib/useSnapshot";
import {
  activityState,
  currency,
  dateTime,
  number,
  percent,
  providerLabel,
  relative,
  shareOf,
  tokens as formatTokens,
} from "@/lib/format";
import { cn } from "@/lib/utils";
import { DEFAULT_RANGE, RANGE_OPTIONS, TOKEN_METRICS, type SessionStat, type Snapshot } from "@/types";
import { Bar, Button, Empty, Spinner, Stat } from "@/components/ui/primitives";
import { MissingDatabase } from "@/components/MissingDatabase";

const TABS = [
  { value: "overview", label: "总览", icon: Gauge },
  { value: "sessions", label: "会话", icon: Activity },
  { value: "models", label: "模型", icon: Boxes },
  { value: "projects", label: "项目", icon: FolderGit2 },
] as const;

type TabValue = (typeof TABS)[number]["value"];

export default function Popover() {
  const [days, setDays] = useState<number | null>(DEFAULT_RANGE);
  const [tab, setTab] = useState<TabValue>("overview");
  const { snapshot, info, loading, initial, error, reload } = useSnapshot(days);

  if (info && !info.exists) {
    return <MissingDatabase info={info} onRetry={reload} />;
  }

  return (
    <div className="flex h-full flex-col">
      <header
        data-tauri-drag-region
        className="flex shrink-0 items-center justify-between gap-2 border-b border-white/8 px-3 py-2"
      >
        <div data-tauri-drag-region className="flex items-baseline gap-2">
          <span className="text-xs font-semibold">OpenCode 用量</span>
          {snapshot && <span className="tnum text-[10px] text-muted">{relative(snapshot.generatedAt)}</span>}
        </div>
        <div className="flex items-center gap-0.5">
          <Button size="icon" onClick={reload} aria-label="刷新" disabled={loading}>
            <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
          </Button>
          <Button size="icon" onClick={() => void openDashboard()} aria-label="打开看板">
            <ExternalLink className="size-3.5" />
          </Button>
          <Button size="icon" onClick={() => void hidePopover()} aria-label="关闭">
            <X className="size-3.5" />
          </Button>
        </div>
      </header>

      <RangePicker value={days} onChange={setDays} />

      {error && <ErrorBar message={error} onRetry={reload} />}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {initial && !snapshot ? (
          <div className="flex h-full items-center justify-center text-muted">
            <Spinner />
          </div>
        ) : snapshot ? (
          <Tabs.Root value={tab} onValueChange={(value) => setTab(value as TabValue)}>
            <Tabs.List className="flex gap-1 border-b border-white/8 px-2 py-1.5">
              {TABS.map(({ value, label, icon: Icon }) => (
                <Tabs.Trigger
                  key={value}
                  value={value}
                  className="flex flex-1 items-center justify-center gap-1 rounded-md py-1 text-[11px] text-muted transition-colors hover:text-foreground data-[state=active]:bg-white/10 data-[state=active]:text-foreground"
                >
                  <Icon className="size-3" />
                  {label}
                </Tabs.Trigger>
              ))}
            </Tabs.List>
            <div className="p-3">
              {tab === "overview" && <Overview snapshot={snapshot} />}
              {tab === "sessions" && <Sessions rows={snapshot.sessions} />}
              {tab === "models" && <Models snapshot={snapshot} />}
              {tab === "projects" && <Projects snapshot={snapshot} />}
            </div>
          </Tabs.Root>
        ) : null}
      </div>
    </div>
  );
}

function RangePicker({ value, onChange }: { value: number | null; onChange: (next: number | null) => void }) {
  return (
    <div className="flex shrink-0 items-center gap-1 border-b border-white/8 px-3 py-1.5">
      {RANGE_OPTIONS.map((option) => (
        <Button
          key={option.label}
          variant={option.days === value ? "solid" : "ghost"}
          onClick={() => onChange(option.days)}
        >
          {option.label}
        </Button>
      ))}
    </div>
  );
}

function ErrorBar({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="flex items-center justify-between gap-2 border-b border-cache-miss/30 bg-cache-miss/10 px-3 py-1.5 text-[11px] text-cache-miss">
      <span className="truncate">{message}</span>
      <Button variant="outline" onClick={onRetry}>
        重试
      </Button>
    </div>
  );
}

function Overview({ snapshot }: { snapshot: Snapshot }) {
  const { overview, today, cache, tools } = snapshot;
  const topTools = tools.slice(0, 6);
  return (
    <div className="flex flex-col gap-3">
      <div className="grid grid-cols-2 gap-2 rounded-md border border-white/8 bg-white/[0.02] p-2">
        <Stat label="今日 Tokens" value={formatTokens(today.tokens.total)} tone="var(--color-token-total)" />
        <Stat label="今日成本" value={currency(today.cost)} />
        <Stat label="总 Tokens" value={formatTokens(overview.tokens.total)} />
        <Stat label="总成本" value={currency(overview.totalCost)} />
      </div>

      <div className="grid grid-cols-2 gap-2">
        <Stat
          label="会话"
          value={number(overview.sessionCount)}
          hint={overview.dayCount > 0 ? `${overview.dayCount} 天跨度` : undefined}
        />
        <Stat
          label="消息"
          value={number(overview.messageCount)}
          hint={`助手 ${number(overview.assistantMessageCount)}`}
        />
        <Stat label="缓存命中" value={percent(overview.cacheHitRate)} tone="var(--color-cache-hit)" />
        <Stat
          label="每会话 Tokens"
          value={formatTokens(overview.avgTokensPerSession)}
          hint={`中位 ${formatTokens(overview.medianTokensPerSession)}`}
        />
      </div>

      <section>
        <h3 className="mb-1.5 text-[10px] tracking-wide text-muted uppercase">Token 构成</h3>
        <div className="flex flex-col gap-1">
          {TOKEN_METRICS.slice(1).map((metric) => {
            const value = overview.tokens[metric.key];
            return (
              <div key={metric.key} className="flex items-center gap-2">
                <span className="w-16 shrink-0 text-[10px] text-muted">{metric.label}</span>
                <Bar ratio={value / Math.max(1, overview.tokens.total)} color={metric.color} />
                <span className="tnum w-14 shrink-0 text-right text-[10px]">
                  {formatTokens(value)}
                </span>
              </div>
            );
          })}
        </div>
      </section>

      <section className="grid grid-cols-2 gap-2 rounded-md border border-white/8 p-2">
        <Stat label="缓存缺口" value={formatTokens(cache.miss)} tone="var(--color-cache-miss)" />
        <Stat label="缺口率" value={percent(cache.missRate)} tone="var(--color-cache-expected)" />
      </section>

      {topTools.length > 0 && (
        <section>
          <h3 className="mb-1.5 text-[10px] tracking-wide text-muted uppercase">工具调用</h3>
          <div className="flex flex-col gap-1">
            {topTools.map((tool) => (
              <div key={tool.name} className="flex items-center gap-2">
                <span className="w-16 shrink-0 truncate text-[10px] text-muted">{tool.name}</span>
                <Bar ratio={tool.percentage / 100} color="var(--color-chart-2)" />
                <span className="tnum w-14 shrink-0 text-right text-[10px]">
                  {number(tool.count)}
                </span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

function Sessions({ rows }: { rows: SessionStat[] }) {
  if (rows.length === 0) return <Empty>窗口内没有会话</Empty>;
  return (
    <ul className="flex flex-col gap-1.5">
      {rows.map((row) => {
        const state = activityState(row.lastUpdated);
        return (
          <li key={row.id} className="rounded-md border border-white/8 p-2">
            <div className="flex items-start justify-between gap-2">
              <span className="line-clamp-1 text-[11px] font-medium">{row.title || row.slug || row.id}</span>
              <span
                className={cn(
                  "mt-0.5 size-1.5 shrink-0 rounded-full",
                  state === "live" && "bg-live",
                  state === "recent" && "bg-recent",
                  state === "idle" && "bg-muted/50",
                )}
                title={dateTime(row.lastUpdated)}
              />
            </div>
            <div className="mt-1 flex items-center justify-between text-[10px] text-muted">
              <span className="truncate">{row.projectName}</span>
              <span className="tnum shrink-0">
                {formatTokens(row.tokens.total)} · {currency(row.cost)}
              </span>
            </div>
            <div className="mt-0.5 flex items-center justify-between text-[10px] text-muted">
              <span className="truncate">
                {row.model || "—"}
                {row.provider ? ` · ${row.provider}` : ""}
              </span>
              <span className="shrink-0">{relative(row.lastUpdated)}</span>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function Models({ snapshot }: { snapshot: Snapshot }) {
  const rows = useMemo(
    () => [...snapshot.models].sort((a, b) => b.tokens.total - a.tokens.total),
    [snapshot.models],
  );
  if (rows.length === 0) return <Empty>窗口内没有模型用量</Empty>;
  const total = rows.reduce((sum, row) => sum + row.tokens.total, 0);
  const totalCost = rows.reduce((sum, row) => sum + row.cost, 0);
  return (
    <ul className="flex flex-col gap-1.5">
      {rows.map((row, index) => (
        <li key={row.name} className="rounded-md border border-white/8 p-2">
          <div className="flex items-baseline justify-between gap-2">
            <span className="truncate text-[11px] font-medium" title={row.name}>
              {row.name.slice(0, 28)}
            </span>
            <span className="tnum shrink-0 text-[10px] text-muted">{shareOf(row.tokens.total, total)}</span>
          </div>
          <div className="mt-1">
            <Bar
              ratio={row.tokens.total / Math.max(1, total)}
              color={`var(--color-chart-${(index % 5) + 1})`}
            />
          </div>
          <div className="mt-1 flex items-center justify-between text-[10px] text-muted">
            <span className="truncate">{providerLabel(row.providers)}</span>
            <span className="tnum shrink-0">
              {formatTokens(row.tokens.total)} · {currency(row.cost)}
            </span>
          </div>
          <div className="tnum mt-0.5 text-[10px] text-muted">
            {number(row.messageCount)} 条消息 · 均 {currency(totalCost > 0 ? row.cost / row.messageCount : 0)}
          </div>
        </li>
      ))}
    </ul>
  );
}

function Projects({ snapshot }: { snapshot: Snapshot }) {
  const rows = useMemo(
    () => [...snapshot.projects].sort((a, b) => b.tokens.total - a.tokens.total),
    [snapshot.projects],
  );
  if (rows.length === 0) return <Empty>窗口内没有项目</Empty>;
  return (
    <ul className="flex flex-col gap-1.5">
      {rows.map((row) => (
        <li key={row.id} className="rounded-md border border-white/8 p-2">
          <div className="flex items-baseline justify-between gap-2">
            <span className="truncate text-[11px] font-medium" title={row.path}>
              {row.name}
            </span>
            <span className="tnum shrink-0 text-[10px] text-muted">
              {formatTokens(row.tokens.total)}
            </span>
          </div>
          <div className="mt-1">
            <Bar ratio={row.tokens.total / Math.max(1, rows[0].tokens.total)} color="var(--color-chart-5)" />
          </div>
          <div className="mt-1 flex items-center justify-between text-[10px] text-muted">
            <span className="truncate">{row.latestSessionTitle ?? "—"}</span>
            <span className="tnum shrink-0">
              {currency(row.cost)} · {number(row.sessionCount)} 会话
            </span>
          </div>
        </li>
      ))}
    </ul>
  );
}
