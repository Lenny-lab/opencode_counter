import { useState } from "react";
import { RefreshCw, X } from "lucide-react";

import { useSnapshot } from "@/lib/useSnapshot";
import { hideDashboard } from "@/lib/ipc";
import { currency, number, percent, tokens as formatTokens, utcOffset, bytes, relative } from "@/lib/format";
import { cn } from "@/lib/utils";
import { RANGE_OPTIONS, DEFAULT_RANGE, TOKEN_METRICS, type TokenMetricKey } from "@/types";
import { Button, Card, Spinner, Stat } from "@/components/ui/primitives";
import { Heatmap } from "@/components/Heatmap";
import { TrendChart } from "@/components/TrendChart";
import { ProviderDonut } from "@/components/ProviderDonut";
import { ModelList, ProjectList, ToolList } from "@/components/Lists";
import { CachePanel } from "@/components/CachePanel";
import { MissingDatabase } from "@/components/MissingDatabase";

/**
 * The full window.
 *
 * Two refresh mechanisms, matching the macOS build: a database-change event
 * that arrives almost immediately, and a slow poll as a backstop for the case
 * where the watcher thread is looking at a file that is being rewritten under
 * it. The popover has no poll because it only exists while someone is reading
 * it.
 */
const POLL_MS = 45_000;

export default function Dashboard() {
  const [days, setDays] = useState<number | null>(DEFAULT_RANGE);
  const [metric, setMetric] = useState<TokenMetricKey>("total");
  const { snapshot, info, loading, initial, error, reload } = useSnapshot(days, POLL_MS);

  const colour = TOKEN_METRICS.find((entry) => entry.key === metric)?.color ?? "var(--color-token-total)";
  const label = TOKEN_METRICS.find((entry) => entry.key === metric)?.label ?? metric;

  if (info && !info.exists) {
    return <MissingDatabase info={info} onRetry={reload} />;
  }

  return (
    <div className="flex h-full flex-col">
      <header
        data-tauri-drag-region
        className="flex shrink-0 items-center gap-3 border-b border-white/8 px-4 py-2.5"
      >
        <div data-tauri-drag-region className="flex items-baseline gap-2">
          <h1 className="text-[13px] font-semibold">OpenCode 用量看板</h1>
          {snapshot && (
            <span className="tnum text-[10px] text-muted">
              {utcOffset()} · {relative(snapshot.generatedAt)}
            </span>
          )}
        </div>

        <div className="ml-auto flex items-center gap-1.5">
          <RangePicker value={days} onChange={setDays} />
          <MetricPicker value={metric} onChange={setMetric} />
          <Button variant="outline" size="icon" onClick={reload} aria-label="刷新" disabled={loading}>
            <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
          </Button>
          <Button
            variant="outline"
            size="icon"
            aria-label="关闭"
            onClick={() => void hideDashboard()}
          >
            <X className="size-3.5" />
          </Button>
        </div>
      </header>

      {error && (
        <div className="flex shrink-0 items-center justify-between gap-2 border-b border-cache-miss/30 bg-cache-miss/10 px-4 py-1.5 text-[11px] text-cache-miss">
          <span className="truncate">{error}</span>
          <Button variant="outline" onClick={reload}>
            重试
          </Button>
        </div>
      )}

      <main className="min-h-0 flex-1 overflow-y-auto p-3">
        {initial && !snapshot ? (
          <div className="flex h-full items-center justify-center text-muted">
            <Spinner />
          </div>
        ) : snapshot ? (
          <div className={cn("flex flex-col gap-3 transition-opacity", loading && "opacity-60")}>
            <section className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
              <Card className="lg:col-span-2">
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  <Stat label="今日 Tokens" value={formatTokens(snapshot.today.tokens.total)} tone="var(--color-token-total)" />
                  <Stat label="今日成本" value={currency(snapshot.today.cost)} />
                  <Stat label="窗口 Tokens" value={formatTokens(snapshot.overview.tokens.total)} />
                  <Stat label="窗口成本" value={currency(snapshot.overview.totalCost)} />
                </div>
              </Card>
              <Card>
                <Stat
                  label="会话"
                  value={number(snapshot.overview.sessionCount)}
                  hint={
                    snapshot.overview.firstSessionDate
                      ? `${snapshot.overview.firstSessionDate} → ${snapshot.overview.lastSessionDate}`
                      : undefined
                  }
                />
              </Card>
              <Card>
                <Stat
                  label="消息"
                  value={number(snapshot.overview.messageCount)}
                  hint={`助手 ${number(snapshot.overview.assistantMessageCount)}`}
                />
              </Card>
              <Card>
                <Stat
                  label="缓存命中率"
                  value={percent(snapshot.overview.cacheHitRate)}
                  tone="var(--color-cache-hit)"
                />
              </Card>
              <Card>
                <Stat
                  label="每会话 Tokens"
                  value={formatTokens(snapshot.overview.avgTokensPerSession)}
                  hint={`中位 ${formatTokens(snapshot.overview.medianTokensPerSession)}`}
                />
              </Card>
            </section>

            <div className="grid gap-3 lg:grid-cols-12">
              <Card title={`活动热力图 · ${label}`} className="lg:col-span-8">
                <Heatmap cells={snapshot.heatmap} metric={metric} />
              </Card>
              <Card title="服务商分布" className="lg:col-span-4">
                <ProviderDonut providers={snapshot.providers} />
              </Card>
            </div>

            <Card title={`每日趋势 · ${label}`}>
              <TrendChart days={snapshot.daysSeries} metric={metric} color={colour} label={label} />
            </Card>

            <div className="grid gap-3 lg:grid-cols-12">
              <Card title={`模型 · ${label}`} className="lg:col-span-5">
                <ModelList models={snapshot.models} metric={metric} />
              </Card>
              <Card title="项目" className="lg:col-span-4">
                <ProjectList projects={snapshot.projects} />
              </Card>
              <Card title="工具调用" className="lg:col-span-3">
                <ToolList tools={snapshot.tools} />
              </Card>
            </div>

            <Card title="缓存分析">
              <CachePanel cache={snapshot.cache} />
            </Card>

            <footer className="flex flex-wrap items-center gap-x-3 gap-y-1 px-1 pb-1 text-[10px] text-muted">
              <span className="font-mono" title={snapshot.database}>
                {snapshot.database}
              </span>
              {info && (
                <>
                  <span>{bytes(info.sizeBytes)}</span>
                  {info.modified && <span>更新于 {info.modified}</span>}
                  {info.layout && <span>布局 {info.layout}</span>}
                </>
              )}
              {info && info.extraDatabases.length > 0 && (
                <span>另有 {info.extraDatabases.length} 个同目录数据库未统计</span>
              )}
            </footer>
          </div>
        ) : null}
      </main>
    </div>
  );
}

function RangePicker({ value, onChange }: { value: number | null; onChange: (next: number | null) => void }) {
  return (
    <div className="flex items-center gap-0.5 rounded-md border border-white/10 p-0.5">
      {RANGE_OPTIONS.map((option) => (
        <button
          key={option.label}
          type="button"
          onClick={() => onChange(option.days)}
          className={cn(
            "rounded px-2 py-1 text-[10px] transition-colors",
            option.days === value
              ? "bg-primary/20 text-primary"
              : "text-muted hover:bg-white/8 hover:text-foreground",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function MetricPicker({
  value,
  onChange,
}: {
  value: TokenMetricKey;
  onChange: (next: TokenMetricKey) => void;
}) {
  return (
    <div className="flex items-center gap-0.5 rounded-md border border-white/10 p-0.5">
      {TOKEN_METRICS.map((metric) => (
        <button
          key={metric.key}
          type="button"
          title={metric.label}
          onClick={() => onChange(metric.key)}
          className={cn(
            "flex items-center gap-1 rounded px-1.5 py-1 text-[10px] transition-colors",
            metric.key === value ? "bg-primary/20 text-foreground" : "text-muted hover:bg-white/8",
          )}
        >
          <span
            className="size-1.5 rounded-full"
            style={{ backgroundColor: metric.color }}
            aria-hidden
          />
          {metric.label}
        </button>
      ))}
    </div>
  );
}
