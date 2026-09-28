import type { DatabaseInfo } from "@/types";
import { bytes } from "@/lib/format";
import { Button } from "@/components/ui/primitives";

/**
 * The "there is no database here" state.
 *
 * Probing happens without opening a connection precisely so this can render
 * instead of every command throwing. It is also the only place worth telling
 * the user where we looked, because on Windows the location depends on
 * environment variables as much as on the user profile.
 */
export function MissingDatabase({ info, onRetry }: { info: DatabaseInfo; onRetry: () => void }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 p-4 text-center">
      <div>
        <p className="text-xs font-semibold">找不到 OpenCode 数据库</p>
        <p className="mt-1 text-[11px] text-muted">
          这个应用只读取 OpenCode 写在磁盘上的消息记录，不连接网络。请确认至少启动过一次
          OpenCode。
        </p>
      </div>
      {info.path && (
        <p className="w-full truncate rounded-md border border-white/8 bg-white/[0.02] px-2 py-1.5 text-left font-mono text-[10px] text-muted">
          {info.path}
        </p>
      )}
      <div className="flex items-center gap-2">
        <Button variant="solid" onClick={onRetry}>
          重新检测
        </Button>
      </div>
      {info.sizeBytes > 0 && <p className="text-[10px] text-muted">{bytes(info.sizeBytes)}</p>}
    </div>
  );
}
