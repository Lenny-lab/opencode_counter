import { number, percent, tokens as formatTokens } from "@/lib/format";
import type { CacheSummary } from "@/types";
import { Empty, Stat } from "@/components/ui/primitives";

/**
 * Why the cache is not doing its job.
 *
 * A miss is only meaningful next to what was cacheable in the first place, so
 * the headline figure is the rate, and the two counters it is computed from sit
 * directly underneath. The composition below is the other half of the story:
 * a session that never reads from cache at all is a different problem from one
 * that reads a little and misses a lot.
 */
export function CachePanel({ cache }: { cache: CacheSummary }) {
  if (cache.expected === 0) {
    return (
      <Empty>
        窗口内没有可缓存的上下文
        <br />
        只有连续使用同一模型的前后消息才会配对，切换模型或压缩历史会中断统计
      </Empty>
    );
  }

  const served = Math.max(0, cache.expected - cache.miss);

  return (
    <div className="flex flex-col gap-3">
      <div className="grid grid-cols-3 gap-2">
        <Stat label="缓存命中" value={percent(cache.hitRate)} tone="var(--color-cache-hit)" />
        <Stat label="缓存缺口" value={formatTokens(cache.miss)} tone="var(--color-cache-miss)" />
        <Stat label="缺口率" value={percent(cache.missRate)} tone="var(--color-cache-expected)" />
      </div>

      <div>
        <div className="mb-1 flex items-center justify-between text-[10px] text-muted">
          <span>缺口</span>
          <span className="tnum">可缓存 {formatTokens(cache.expected)}</span>
        </div>
        <div className="flex h-2 w-full overflow-hidden rounded-full bg-white/8">
          <div
            className="h-full bg-cache-hit"
            style={{ width: `${(served / cache.expected) * 100}%` }}
          />
          <div
            className="h-full bg-cache-miss"
            style={{ width: `${(cache.miss / cache.expected) * 100}%` }}
          />
        </div>
        <div className="mt-1 flex items-center justify-between text-[10px] text-muted">
          <span className="tnum">命中 {formatTokens(served)}</span>
          <span className="tnum">缺口 {formatTokens(cache.miss)}</span>
        </div>
      </div>

      {cache.noCacheSessions > 0 && (
        <p className="rounded-md border border-cache-expected/25 bg-cache-expected/8 px-2 py-1.5 text-[10px] text-cache-expected">
          {number(cache.noCacheSessions)} 个会话一次都没命中缓存，通常意味着首轮请求就带上了完整历史，
          或该服务商不支持提示缓存。
        </p>
      )}
    </div>
  );
}
