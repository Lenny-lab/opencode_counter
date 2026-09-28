/**
 * Mirrors the `serde` output of the Rust side.
 *
 * Every struct there is `#[serde(rename_all = "camelCase")]`, so these names
 * are the wire contract, not a convenience. If a field moves in Rust, it moves
 * here too and `npm run build` will say so.
 */

export interface TokenBreakdown {
  input: number;
  output: number;
  reasoning: number;
  cacheRead: number;
  cacheWrite: number;
  /** Always the sum of the five above; never read from the database. */
  total: number;
}

export interface TodayStat {
  cost: number;
  tokens: TokenBreakdown;
}

export interface Overview {
  sessionCount: number;
  messageCount: number;
  assistantMessageCount: number;
  /** Inclusive calendar days spanned by the sessions in range, not active days. */
  dayCount: number;
  firstSessionDate: string | null;
  lastSessionDate: string | null;
  totalCost: number;
  avgCostPerDay: number;
  tokens: TokenBreakdown;
  /** 0-100. */
  cacheHitRate: number;
  /** Reasoning is excluded from these two, by long-standing convention. */
  avgTokensPerSession: number;
  medianTokensPerSession: number;
}

export interface DayStat {
  /** `YYYY-MM-DD` in local time. */
  date: string;
  messageCount: number;
  userMessageCount: number;
  cost: number;
  tokens: TokenBreakdown;
  cacheMiss: number;
  cacheExpected: number;
  cacheMissRate: number;
}

export interface HeatCell {
  date: string;
  /** Hour the two-hour bucket starts at: 0, 2, ... 22. */
  block: number;
  messageCount: number;
  userMessageCount: number;
  cost: number;
  tokens: TokenBreakdown;
}

export interface ModelStat {
  name: string;
  /** Every provider seen serving this model, sorted and deduplicated. */
  providers: string[];
  messageCount: number;
  cost: number;
  tokens: TokenBreakdown;
}

export interface ProviderStat {
  name: string;
  messageCount: number;
  cost: number;
  tokens: TokenBreakdown;
}

export interface ProjectStat {
  id: string;
  name: string;
  path: string;
  sessionCount: number;
  messageCount: number;
  cost: number;
  tokens: TokenBreakdown;
  latestSessionId: string | null;
  latestSessionTitle: string | null;
  latestSessionUpdatedAt: string | null;
}

export interface ToolStat {
  name: string;
  count: number;
  percentage: number;
}

export interface SessionStat {
  id: string;
  title: string;
  projectName: string;
  path: string;
  slug: string;
  cost: number;
  messageCount: number;
  provider: string;
  model: string;
  lastUpdated: string | null;
  tokens: TokenBreakdown;
}

export interface CacheSummary {
  /** Tokens a warm predecessor should have cached but did not. */
  miss: number;
  /** Tokens that were cacheable in principle. */
  expected: number;
  missRate: number;
  /** Share of billable input served from cache, 0-100. */
  hitRate: number;
  noCacheSessions: number;
}

export interface Snapshot {
  generatedAt: string;
  /** The window in days. `null` means all time. */
  days: number | null;
  database: string;
  today: TodayStat;
  overview: Overview;
  daysSeries: DayStat[];
  heatmap: HeatCell[];
  models: ModelStat[];
  providers: ProviderStat[];
  projects: ProjectStat[];
  tools: ToolStat[];
  sessions: SessionStat[];
  cache: CacheSummary;
}

export interface DatabaseInfo {
  path: string;
  exists: boolean;
  sizeBytes: number;
  modified: string | null;
  dataDir: string;
  layout: string | null;
  extraDatabases: string[];
}

/** Options for the window selector. `null` means all time. */
export const RANGE_OPTIONS: { label: string; days: number | null }[] = [
  { label: "今天", days: 1 },
  { label: "7 天", days: 7 },
  { label: "30 天", days: 30 },
  { label: "90 天", days: 90 },
  { label: "一年", days: 365 },
  { label: "全部", days: null },
];

export const DEFAULT_RANGE = 30;

/** Token counters that can be plotted, with the palette the macOS build used. */
export const TOKEN_METRICS = [
  { key: "total", label: "总 Tokens", color: "#86A8FF" },
  { key: "input", label: "输入", color: "#7AD7FF" },
  { key: "output", label: "输出", color: "#F3B56F" },
  { key: "reasoning", label: "推理", color: "#FF8CC6" },
  { key: "cacheRead", label: "缓存读取", color: "#9E8CFF" },
  { key: "cacheWrite", label: "缓存写入", color: "#FF9E6E" },
] as const;

export type TokenMetricKey = (typeof TOKEN_METRICS)[number]["key"];
