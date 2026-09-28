//
//  DashboardView.swift
//  OpenCode Stats
//
//  Native desktop dashboard — a 1:1 SwiftUI port of the web dashboard
//  (same layout, palette and content; no browser, no local HTTP server).
//

import SwiftUI
import Charts

// MARK: - Palette (converted from the web dark theme, src/index.css `.dark`)

extension Color {
    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

enum D {
    static let background = Color(hex: 0x04060D)
    static let foreground = Color(hex: 0xEDEEF2)
    static let muted = Color(hex: 0x888F9F)
    static let primary = Color(hex: 0x6492FF)
    static let chart1 = Color(hex: 0x5D90FF)
    static let chart2 = Color(hex: 0x00C59E)
    static let chart3 = Color(hex: 0xE49E22)
    static let chart4 = Color(hex: 0xD54ECD)
    static let chart5 = Color(hex: 0x886BEE)

    static let panel = Color.white.opacity(0.035)
    static let panelBorder = Color.white.opacity(0.10)
    static let hairline = Color.white.opacity(0.09)
    static let subtleBorder = Color.white.opacity(0.12)
    static let secondaryFill = Color(hex: 0x0A0D14)
    static let cellEmpty = Color.white.opacity(0.07)
}

enum DashboardDateRange {
    static let label = "最近 12 个月"

    static var dayCount: Int {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: Date())
        let currentMonthStart = calendar.date(
            from: calendar.dateComponents([.year, .month], from: today)
        ) ?? today
        let start = calendar.date(byAdding: .month, value: -11, to: currentMonthStart) ?? today
        return (calendar.dateComponents([.day], from: start, to: today).day ?? 0) + 1
    }
}

// MARK: - Metric selector (subset of the web METRIC_OPTIONS)

enum DashMetric: String, CaseIterable, Identifiable {
    case total
    case input
    case output
    case reasoning
    case cacheRead = "cache_read"
    case cacheWrite = "cache_write"
    case userMessageCount = "user_message_count"

    var id: String { rawValue }

    var label: String {
        switch self {
        case .total: "总 Tokens"
        case .input: "输入 Tokens"
        case .output: "输出 Tokens"
        case .reasoning: "推理 Tokens"
        case .cacheRead: "缓存读取"
        case .cacheWrite: "缓存写入"
        case .userMessageCount: "用户消息数"
        }
    }

    /// Colors from the web `METRIC_META`.
    var color: Color {
        switch self {
        case .total: Color(hex: 0x86A8FF)
        case .input: Color(hex: 0x7AD7FF)
        case .output: Color(hex: 0xF3B56F)
        case .reasoning: Color(hex: 0xFF8CC6)
        case .cacheRead: Color(hex: 0x9E8CFF)
        case .cacheWrite: Color(hex: 0xFF9E6E)
        case .userMessageCount: Color(hex: 0xB7EF6D)
        }
    }

    var isCount: Bool { self == .userMessageCount }

    func value(_ day: DayAgg) -> Int64 {
        switch self {
        case .total: day.total
        case .input: day.input
        case .output: day.output
        case .reasoning: day.reasoning
        case .cacheRead: day.cacheRead
        case .cacheWrite: day.cacheWrite
        case .userMessageCount: Int64(day.userCount)
        }
    }

    func value(_ cell: HeatmapCell) -> Int64 {
        switch self {
        case .total: cell.tokens
        case .input: cell.input
        case .output: cell.output
        case .reasoning: cell.reasoning
        case .cacheRead: cell.cacheRead
        case .cacheWrite: cell.cacheWrite
        case .userMessageCount: Int64(cell.userCount)
        }
    }

    func value(_ model: ModelUsageItem) -> Int64 {
        switch self {
        case .total: model.totalTokens
        case .input: model.inputTokens
        case .output: model.outputTokens
        case .reasoning: model.reasoningTokens
        case .cacheRead: model.cacheRead
        case .cacheWrite: model.cacheWrite
        case .userMessageCount: 0
        }
    }

    func value(_ provider: ProviderUsageItem) -> Int64 {
        switch self {
        case .total: provider.totalTokens
        case .input: provider.inputTokens
        case .output: provider.outputTokens
        case .reasoning: provider.reasoningTokens
        case .cacheRead: provider.cacheRead
        case .cacheWrite: provider.cacheWrite
        case .userMessageCount: 0
        }
    }
}

// MARK: - Aggregated series (mirrors the web payload `days` + `heatmap`)

struct DayAgg: Identifiable {
    let day: String
    var input: Int64 = 0
    var output: Int64 = 0
    var reasoning: Int64 = 0
    var cacheRead: Int64 = 0
    var cacheWrite: Int64 = 0
    var messageCount: Int = 0
    var userCount: Int = 0

    var id: String { day }
    var total: Int64 { input + output + reasoning + cacheRead + cacheWrite }
    var inputSide: Int64 { input + cacheRead + cacheWrite }
    var outputSide: Int64 { output + reasoning }
}

struct UsageSeries {
    let days: [DayAgg]
    let blocks: [String: HeatmapCell]
    let today: String

    init(cells: [HeatmapCell], daysFilter: Int?) {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        let cal = Calendar.current
        let todayStr = fmt.string(from: Date())

        var agg: [String: DayAgg] = [:]
        var blockMap: [String: HeatmapCell] = [:]
        for cell in cells {
            var day = agg[cell.day] ?? DayAgg(day: cell.day)
            day.input += cell.input
            day.output += cell.output
            day.reasoning += cell.reasoning
            day.cacheRead += cell.cacheRead
            day.cacheWrite += cell.cacheWrite
            day.messageCount += cell.messageCount
            day.userCount += cell.userCount
            agg[cell.day] = day
            blockMap[cell.id] = cell
        }

        var dayKeys: [String] = agg.keys.sorted()
        if let filter = daysFilter {
            // Selected range: exactly `filter` calendar days ending today (zero-filled).
            let start = cal.date(byAdding: .day, value: -(filter - 1), to: cal.startOfDay(for: Date())) ?? Date()
            var keys: [String] = []
            var current = start
            while current <= Date() && keys.count < 400 {
                keys.append(fmt.string(from: current))
                current = cal.date(byAdding: .day, value: 1, to: current) ?? current.addingTimeInterval(86_400)
            }
            dayKeys = keys
        } else if let first = dayKeys.first, let last = dayKeys.last,
                  var current = Self.parse(first, fmt: fmt) {
            let end = Self.parse(last, fmt: fmt) ?? current
            var keys: [String] = []
            while current <= end && keys.count < 400 {
                keys.append(fmt.string(from: current))
                current = cal.date(byAdding: .day, value: 1, to: current) ?? current.addingTimeInterval(86_400)
            }
            dayKeys = keys
        }

        self.days = dayKeys.map { agg[$0] ?? DayAgg(day: $0) }
        self.blocks = blockMap
        self.today = todayStr
    }

    private static func parse(_ day: String, fmt: DateFormatter) -> Date? {
        fmt.date(from: day)
    }

    func total(_ metric: DashMetric) -> Int64 {
        days.reduce(0) { $0 + metric.value($1) }
    }

    var inputTotal: Int64 { days.reduce(0) { $0 + $1.input } }
    var outputTotal: Int64 { days.reduce(0) { $0 + $1.output } }
    var reasoningTotal: Int64 { days.reduce(0) { $0 + $1.reasoning } }
    var cacheReadTotal: Int64 { days.reduce(0) { $0 + $1.cacheRead } }
    var cacheWriteTotal: Int64 { days.reduce(0) { $0 + $1.cacheWrite } }
    var userTotal: Int { days.reduce(0) { $0 + $1.userCount } }

    var dayCount: Int { days.count }

    var average: Int64 {
        guard dayCount > 0 else { return 0 }
        return total(currentMetric) / Int64(dayCount)
    }

    /// Set by the view before computing summary cards.
    var currentMetric: DashMetric = .total

    func peak(_ metric: DashMetric) -> DayAgg? {
        days.filter { metric.value($0) > 0 }.max { metric.value($0) < metric.value($1) }
    }

    func lastNonZero(_ metric: DashMetric) -> DayAgg? {
        days.last { metric.value($0) > 0 }
    }

    /// Web `recomputeCacheHitRate`: cache_read ÷ (input + cache_read + cache_write).
    func hitRate(on day: DayAgg) -> Double {
        let total = day.input + day.cacheRead + day.cacheWrite
        guard total > 0 else { return 0 }
        return Double(day.cacheRead) / Double(total) * 100
    }

    var cacheHitRate: Double {
        let total = inputTotal + cacheReadTotal + cacheWriteTotal
        guard total > 0 else { return 0 }
        return Double(cacheReadTotal) / Double(total) * 100
    }
}

// MARK: - Shared card chrome

struct DashboardSectionHeader: View {
    let eyebrow: String
    let title: String
    var subtitle: String? = nil
    var eyebrowColor: Color = D.chart2

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(eyebrow)
                .font(.system(size: 10, weight: .semibold))
                .tracking(1.4)
                .textCase(.uppercase)
                .foregroundStyle(eyebrowColor)
            Text(title)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(D.foreground)
            if let subtitle {
                Text(subtitle)
                    .font(.system(size: 12))
                    .foregroundStyle(D.muted)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// Web `.glass-panel` card: translucent dark panel + 10% hairline ring.
struct DashboardCard<Content: View>: View {
    var padding: CGFloat = 16
    var radius: CGFloat = 12
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            content
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(padding)
        .background(
            RoundedRectangle(cornerRadius: radius, style: .continuous)
                .fill(D.panel)
        )
        .overlay(
            RoundedRectangle(cornerRadius: radius, style: .continuous)
                .strokeBorder(D.panelBorder, lineWidth: 1)
        )
    }
}

// MARK: - Dashboard window

struct DashboardView: View {
    @ObservedObject private var db = OpenCodeDatabase.shared
    @State private var metric: DashMetric = .total
    @State private var showExport = false

    private var series: UsageSeries {
        var s = UsageSeries(cells: db.stats.heatmap, daysFilter: DashboardDateRange.dayCount)
        s.currentMetric = metric
        return s
    }

    var body: some View {
        Group {
            if !db.dbExists {
                unavailableView(
                    icon: "externaldrive.triangle.badge.exclamationmark",
                    title: "未找到 OpenCode 数据库",
                    message: "请确认已安装 OpenCode 并至少运行过一次。"
                )
            } else if let error = db.error {
                unavailableView(
                    icon: "exclamationmark.triangle.fill",
                    title: "数据加载失败",
                    message: error
                )
            } else {
                dashboard
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(D.background)
        .preferredColorScheme(.dark)
        .onAppear {
            let annualDays = DashboardDateRange.dayCount
            if db.daysFilter != annualDays {
                db.daysFilter = annualDays
                db.refresh()
            } else if db.stats.heatmap.isEmpty && !db.isLoading {
                db.refresh()
            }
        }
    }

    private var dashboard: some View {
        let series = self.series
        return ScrollView {
            VStack(spacing: 0) {
                VStack(spacing: 0) {
                    hero(series: series)
                        .padding(.top, 4)
                    controls
                        .padding(.top, 12)
                    gradientSeparator
                        .padding(.top, 16)
                        .padding(.bottom, 4)
                    summaryCards(series: series)
                        .padding(.top, 8)
                    chartsGrid(series: series)
                        .padding(.top, 12)
                    footer
                        .padding(.top, 16)
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 16)
                .frame(maxWidth: 1600)
                .frame(maxWidth: .infinity, alignment: .center)
            }
            .background(DashboardBackground())
        }
        .background(D.background)
    }

    // MARK: Hero

    private func hero(series: UsageSeries) -> some View {
        let firstDay = series.days.first?.day
        let lastDay = series.days.last?.day
        let rangeText: String = {
            if let first = firstDay, let last = lastDay {
                return "\(Formatters.yearMonthDay(first)) — \(Formatters.yearMonthDay(last))"
            }
            return "—"
        }()

        return VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top, spacing: 20) {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        Image(systemName: "waveform.path.ecg")
                            .font(.system(size: 11))
                        Text("OpenCode Usage Monitor")
                            .font(.system(size: 10, weight: .semibold))
                            .tracking(1.4)
                            .textCase(.uppercase)
                    }
                    .foregroundStyle(D.chart2)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 3)
                    .background(
                        Capsule().fill(D.chart2.opacity(0.05))
                    )
                    .overlay(
                        Capsule().strokeBorder(D.chart2.opacity(0.20), lineWidth: 1)
                    )

                    HStack(spacing: 8) {
                        Text("Tokens")
                            .foregroundStyle(D.foreground)
                        Text("用量看板")
                            .foregroundStyle(
                                LinearGradient(
                                    colors: [D.primary, D.chart2],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                )
                            )
                    }
                    .font(.system(size: 34, weight: .bold))
                    .tracking(-0.5)

                    Text("交互式图表呈现 Token 消耗趋势与模型用量")
                        .font(.system(size: 14))
                        .foregroundStyle(D.muted)
                        .padding(.top, 2)
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                VStack(spacing: 8) {
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible())], spacing: 8) {
                        metaCard(
                            icon: "calendar",
                            color: D.chart2,
                            label: "统计区间",
                            value: rangeText
                        )
                        metaCard(
                            icon: "waveform.path.ecg",
                            color: D.primary,
                            label: "当前范围",
                            value: DashboardDateRange.label
                        )
                        metaCard(
                            icon: "number",
                            color: D.chart3,
                            label: "assistant 消息",
                            value: Formatters.full(Int64(db.stats.assistantMessageCount))
                        )
                        metaCard(
                            icon: "waveform.path.ecg",
                            color: D.chart4,
                            label: "当前范围 \(metric.label)",
                            value: Formatters.full(series.total(metric))
                        )
                    }
                }
                .frame(width: 430)
            }

            HStack(spacing: 6) {
                ZStack {
                    Circle().fill(Color.green.opacity(0.35)).frame(width: 8, height: 8)
                    Circle().fill(Color(hex: 0x10B981)).frame(width: 6, height: 6)
                }
                Text("更新于 \(Formatters.monthDayTime(Date())) · \(Formatters.utcOffset())")
                    .font(.system(size: 11))
                    .foregroundStyle(D.muted)
            }
            .padding(.top, 12)
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(D.panel)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(D.panelBorder, lineWidth: 1)
        )
    }

    private func metaCard(icon: String, color: Color, label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.system(size: 11))
                    .foregroundStyle(color)
                Text(label)
                    .font(.system(size: 11))
                    .foregroundStyle(D.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            Text(value)
                .font(.system(size: 13, weight: .semibold, design: .monospaced))
                .foregroundStyle(D.foreground)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(D.secondaryFill.opacity(0.6))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(D.panelBorder, lineWidth: 1)
        )
    }

    // MARK: Controls

    private var controls: some View {
        HStack(spacing: 10) {
            Spacer(minLength: 12)

            Menu {
                ForEach(DashMetric.allCases) { option in
                    Button {
                        metric = option
                    } label: {
                        if option == metric {
                            Label(option.label, systemImage: "checkmark")
                        } else {
                            Text(option.label)
                        }
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    Text(metric.label)
                        .font(.system(size: 13))
                        .foregroundStyle(D.foreground)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(D.muted)
                }
                .padding(.horizontal, 10)
                .frame(height: 32)
                .frame(minWidth: 150)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Color.black.opacity(0.25))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(D.subtleBorder, lineWidth: 1)
                )
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel("指标 \(metric.label)")

            Button {
                db.daysFilter = DashboardDateRange.dayCount
                db.refresh()
            } label: {
                HStack(spacing: 6) {
                    if db.isLoading {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: "arrow.clockwise")
                            .font(.system(size: 11))
                    }
                    Text(db.isLoading ? "加载中…" : "刷新")
                        .font(.system(size: 12))
                }
                .foregroundStyle(D.foreground)
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Color.black.opacity(0.25))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(D.subtleBorder, lineWidth: 1)
                )
            }
            .buttonStyle(.plain)
            .accessibilityLabel("刷新数据")

            Button {
                showExport = true
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "square.and.arrow.down")
                        .font(.system(size: 11))
                    Text("导出 CSV")
                        .font(.system(size: 12))
                }
                .foregroundStyle(D.foreground)
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Color.black.opacity(0.25))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(D.subtleBorder, lineWidth: 1)
                )
            }
            .buttonStyle(.plain)
            .accessibilityLabel("导出 CSV")
        }
        .sheet(isPresented: $showExport) {
            ExportSheet()
        }
    }

    private var gradientSeparator: some View {
        Rectangle()
            .fill(
                LinearGradient(
                    colors: [.clear, D.hairline, .clear],
                    startPoint: .leading,
                    endPoint: .trailing
                )
            )
            .frame(height: 1)
    }

    // MARK: Summary cards

    private func summaryCards(series: UsageSeries) -> some View {
        LazyVGrid(
            columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 5),
            spacing: 8
        ) {
            let latest = series.lastNonZero(metric)
            let isToday = latest?.day == series.today
            let peak = series.peak(metric)
            let avg = series.dayCount > 0 ? series.total(metric) / Int64(series.dayCount) : 0
            summaryCard(
                label: latest == nil ? "暂无数据" : (isToday ? "今天" : "最近一天"),
                value: latest.map { Formatters.full(metric.value($0)) } ?? "0",
                subtitle: latest.map { "\(metric.label) · \(Formatters.monthDay($0.day))" } ?? metric.label,
                icon: isToday ? "clock" : "calendar",
                accent: D.chart1
            )
            summaryCard(
                label: "合计",
                value: Formatters.full(series.total(metric)),
                subtitle: DashboardDateRange.label,
                icon: "chart.bar",
                accent: D.chart2
            )
            summaryCard(
                label: "日均",
                value: Formatters.full(avg),
                subtitle: metric.label,
                icon: "trendingup",
                accent: D.chart3
            )
            summaryCard(
                label: "峰值",
                value: peak.map { Formatters.full(metric.value($0)) } ?? "0",
                subtitle: peak.map { Formatters.monthDay($0.day) } ?? "暂无数据",
                icon: "arrow.up",
                accent: D.chart4
            )
            utilityCard(series: series)
        }
    }

    @ViewBuilder
    private func utilityCard(series: UsageSeries) -> some View {
        if metric == .userMessageCount {
            summaryCard(
                label: "总 Tokens",
                value: Formatters.full(series.total(.total)),
                subtitle: "当前范围合计",
                icon: "bolt.fill",
                accent: D.chart3
            )
        } else {
            let perDay = series.dayCount > 0 ? series.userTotal / series.dayCount : 0
            summaryCard(
                label: "用户消息",
                value: Formatters.full(Int64(series.userTotal)),
                subtitle: "日均 \(Formatters.full(Int64(perDay))) 条",
                icon: "message",
                accent: D.chart4
            )
        }
    }

    private func summaryCard(label: String, value: String, subtitle: String, icon: String, accent: Color) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Rectangle()
                .fill(accent)
                .frame(height: 2)
                .opacity(0.5)
                .padding(.top, -12)

            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(label)
                        .font(.system(size: 12))
                        .foregroundStyle(D.muted)
                        .lineLimit(1)
                    Spacer()
                    Image(systemName: icon)
                        .font(.system(size: 12))
                        .foregroundStyle(accent)
                        .frame(width: 28, height: 28)
                        .background(Circle().fill(Color.white.opacity(0.05)))
                        .opacity(0.7)
                }
                Text(value)
                    .font(.system(size: 19, weight: .bold, design: .monospaced))
                    .foregroundStyle(D.foreground)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                Text(subtitle)
                    .font(.system(size: 12))
                    .foregroundStyle(D.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(D.panel)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(D.panelBorder, lineWidth: 1)
        )
    }

    // MARK: Charts grid (web lg:grid-cols-12)

    private func chartsGrid(series: UsageSeries) -> some View {
        VStack(spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                HeatmapCard(metric: metric, series: series)
                    .frame(maxWidth: .infinity)
                CompositionCard(series: series)
                    .frame(width: 360)
            }

            TrendCard(metric: metric, series: series)

            HStack(alignment: .top, spacing: 12) {
                ModelCard(metric: metric, models: db.stats.modelUsage)
                    .frame(maxWidth: .infinity)
                ProviderCard(metric: metric, providers: db.stats.providerUsage)
                    .frame(width: 430)
            }

            CacheCard(series: series)

            HStack(alignment: .top, spacing: 12) {
                ProjectsCard(projects: db.stats.projects)
                    .frame(maxWidth: .infinity)
                ToolsCard(tools: db.stats.toolUsage)
                    .frame(width: 430)
            }
        }
    }

    // MARK: Footer

    private var footer: some View {
        VStack(spacing: 10) {
            Rectangle()
                .fill(D.hairline)
                .frame(height: 1)
            HStack(spacing: 8) {
                Text("数据库:")
                Text(db.dbPath)
                    .monospaced()
                Text("·")
                Text("\(Formatters.full(Int64(db.stats.messageCount))) 行已扫描")
            }
            .font(.system(size: 11))
            .foregroundStyle(D.muted)
            .lineLimit(1)
            .minimumScaleFactor(0.6)
        }
        .frame(maxWidth: .infinity)
    }

    private func unavailableView(icon: String, title: String, message: String) -> some View {
        VStack(spacing: 10) {
            Spacer()
            Image(systemName: icon)
                .font(.system(size: 34))
                .foregroundStyle(D.muted)
            Text(title)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(D.foreground)
            Text(message)
                .font(.system(size: 12))
                .foregroundStyle(D.muted)
                .multilineTextAlignment(.center)
            Button("重试") { db.refresh() }
                .buttonStyle(.bordered)
                .tint(D.primary)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
        .background(D.background)
        .preferredColorScheme(.dark)
    }
}

// MARK: - Background (radial glows + grid, web `.bg-radial-glow` / `.bg-grid`)

struct DashboardBackground: View {
    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                D.background

                glow(D.chart2, opacity: 0.08, w: geo.size.width * 0.35, h: geo.size.height * 0.25, x: 0.12, y: 0.08, size: geo.size)
                glow(D.primary, opacity: 0.10, w: geo.size.width * 0.30, h: geo.size.height * 0.20, x: 0.88, y: 0.04, size: geo.size)
                glow(D.chart5, opacity: 0.05, w: geo.size.width * 0.45, h: geo.size.height * 0.35, x: 0.65, y: 0.75, size: geo.size)
                glow(D.chart4, opacity: 0.04, w: geo.size.width * 0.40, h: geo.size.height * 0.30, x: 0.50, y: 0.85, size: geo.size)

                DashboardGrid()
                    .opacity(0.15)
            }
            .frame(width: geo.size.width, height: geo.size.height)
        }
        .ignoresSafeArea()
    }

    private func glow(_ color: Color, opacity: Double, w: CGFloat, h: CGFloat, x: CGFloat, y: CGFloat, size: CGSize) -> some View {
        Ellipse()
            .fill(color.opacity(opacity))
            .frame(width: max(80, w), height: max(60, h))
            .blur(radius: 70)
            .position(x: size.width * x, y: size.height * y)
    }
}

struct DashboardGrid: View {
    var body: some View {
        Canvas { context, size in
            let step: CGFloat = 48
            var path = Path()
            var x: CGFloat = 0
            while x <= size.width {
                path.move(to: CGPoint(x: x, y: 0))
                path.addLine(to: CGPoint(x: x, y: size.height))
                x += step
            }
            var y: CGFloat = 0
            while y <= size.height {
                path.move(to: CGPoint(x: 0, y: y))
                path.addLine(to: CGPoint(x: size.width, y: y))
                y += step
            }
            context.stroke(path, with: .color(D.hairline), lineWidth: 1)
        }
        .mask(
            GeometryReader { geo in
                Ellipse()
                    .fill(
                        RadialGradient(
                            colors: [.white, .white.opacity(0.6), .clear],
                            center: .center,
                            startRadius: 0,
                            endRadius: max(geo.size.width, geo.size.height) * 0.55
                        )
                    )
                    .frame(width: geo.size.width, height: geo.size.height * 0.9)
                    .position(x: geo.size.width * 0.5, y: geo.size.height * 0.3)
            }
        )
    }
}

// MARK: - Heatmap (ACTIVITY)

struct HeatmapCard: View {
    let metric: DashMetric
    let series: UsageSeries

    private struct HoverTip: Equatable {
        let id: String
        let title: String
        let value: String
    }

    @State private var hover: HoverTip?
    /// Harness-only: force a hovered day id (yyyy-MM-dd) for rendering.
    var previewHover: String? = nil

    init(metric: DashMetric, series: UsageSeries, previewHover: String? = nil) {
        self.metric = metric
        self.series = series
        self.previewHover = previewHover
        self._hover = State(initialValue: previewHover.flatMap { Self.makeTip(id: $0, metric: metric, series: series) })
    }

    private static func makeTip(id: String, metric: DashMetric, series: UsageSeries) -> HoverTip? {
        guard let daySeries = series.days.first(where: { $0.day == id }) else { return nil }
        return HoverTip(id: id, title: id, value: "\(Formatters.full(metric.value(daySeries))) \(metric.label)")
    }

    private var rangeTotal: Int64 { series.total(metric) }

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 10) {
                DashboardSectionHeader(
                    eyebrow: "Activity",
                    title: "活跃度热力图",
                    subtitle: "\(metric.label) 活跃度，颜色越深越活跃",
                    eyebrowColor: D.chart3
                )

                if series.days.isEmpty {
                    Text("暂无趋势数据，试试调整时间范围")
                        .font(.system(size: 13))
                        .foregroundStyle(D.muted)
                        .frame(maxWidth: .infinity, minHeight: 80)
                } else {
                    calendarGrid
                }

                legend
            }
        }
    }

    static let LEVEL_PCT: [Double] = [0, 22, 42, 62, 82, 100]

    static func level(value: Int64, maxValue: Int64) -> Int {
        guard value > 0, maxValue > 0 else { return 0 }
        let ratio = sqrt(Double(value) / Double(maxValue))
        return min(5, max(1, Int(ceil(ratio * 5))))
    }

    // MARK: calendar grid (weeks × weekdays, one cell per day)

    private struct CalendarDay: Identifiable {
        let date: String
        let value: Int64
        let inRange: Bool
        var id: String { date }
    }

    private var calendarGrid: some View {
        let weeks = calendarWeeks
        guard !weeks.isEmpty else { return AnyView(EmptyView()) }
        let gap: CGFloat = 3
        let monthH: CGFloat = 14
        let maxValue = series.days.map { metric.value($0) }.max() ?? 0

        return AnyView(
            GeometryReader { geo in
                let gapWidth = CGFloat(max(0, weeks.count - 1)) * gap
                let fittedSize = (geo.size.width - gapWidth) / CGFloat(weeks.count)
                let size = min(20, max(8, fittedSize))
                let gridWidth = CGFloat(weeks.count) * size + gapWidth
                let labels = monthLabels(weeks: weeks, size: size, gap: gap)

                let grid = VStack(alignment: .leading, spacing: gap) {
                    ForEach(0..<7, id: \.self) { weekday in
                        HStack(spacing: gap) {
                            ForEach(Array(weeks.enumerated()), id: \.offset) { weekIndex, week in
                                if weekday < week.count, let day = week[weekday] {
                                    calendarCell(
                                        day: day,
                                        size: size,
                                        maxValue: maxValue,
                                        weekIndex: weekIndex,
                                        weekCount: weeks.count
                                    )
                                } else {
                                    Color.clear.frame(width: size, height: size)
                                }
                            }
                        }
                    }

                    HStack(spacing: gap) {
                        ForEach(Array(labels.enumerated()), id: \.offset) { _, label in
                            Text(label)
                                .font(.system(size: 9, weight: .medium))
                                .foregroundStyle(D.muted)
                                .fixedSize()
                                .frame(width: size, height: monthH, alignment: .topLeading)
                        }
                    }
                }

                if gridWidth <= geo.size.width {
                    HStack(spacing: 0) {
                        grid
                            .frame(width: gridWidth, alignment: .leading)
                        Spacer(minLength: 0)
                    }
                } else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        grid.frame(width: gridWidth, alignment: .leading)
                    }
                }
            }
            // The maximum 20pt cell size determines the stable card height.
            // Smaller long-range cells remain top-aligned without changing the card layout.
            .frame(height: 7 * 20 + 6 * gap + gap + monthH)
        )
    }

    @ViewBuilder
    private func calendarCell(
        day: CalendarDay,
        size: CGFloat,
        maxValue: Int64,
        weekIndex: Int,
        weekCount: Int
    ) -> some View {
        if !day.inRange {
            // Out-of-range days are transparent placeholders (web renders no border).
            Color.clear.frame(width: size, height: size)
        } else {
            let level = Self.level(value: day.value, maxValue: maxValue)
            let isHovered = hover?.id == day.date
            let tooltipAlignment: Alignment = weekIndex == 0
                ? .topLeading
                : (weekIndex == weekCount - 1 ? .topTrailing : .top)
            RoundedRectangle(cornerRadius: 3, style: .continuous)
                .fill(level == 0 ? D.cellEmpty : D.primary.opacity(Self.LEVEL_PCT[level] / 100))
                .frame(width: size, height: size)
                .overlay(
                    RoundedRectangle(cornerRadius: 3, style: .continuous)
                        .strokeBorder(D.hairline.opacity(0.4), lineWidth: 1)
                )
                .onHover { inside in
                    if inside {
                        hover = Self.makeTip(id: day.date, metric: metric, series: series)
                    } else if hover?.id == day.date {
                        hover = nil
                    }
                }
                .overlay(alignment: tooltipAlignment) {
                    if isHovered {
                        tooltipBubble(tip: hover)
                    }
                }
                .zIndex(isHovered ? 30 : 0)
                .animation(.easeOut(duration: 0.12), value: hover?.id)
        }
    }

    @ViewBuilder
    private func tooltipBubble(tip: HoverTip?) -> some View {
        if let tip {
            VStack(alignment: .leading, spacing: 2) {
                Text(tip.title)
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(D.muted)
                Text(tip.value)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(D.foreground)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(Color.black.opacity(0.92))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .strokeBorder(D.hairline, lineWidth: 1)
            )
            .shadow(color: .black.opacity(0.35), radius: 8, y: 4)
            .allowsHitTesting(false)
            .fixedSize()
            // Keep the bubble fully above the hovered cell so the pointer never
            // covers the date or value text.
            .offset(y: -52)
        }
    }

    private var calendarWeeks: [[CalendarDay?]] {
        let cal = Calendar.current
        let fmt = Formatters.day
        guard let firstStr = series.days.first?.day,
              let lastStr = series.days.last?.day,
              let firstDate = fmt.date(from: firstStr),
              let lastDate = fmt.date(from: lastStr)
        else { return [] }

        let values = Dictionary(uniqueKeysWithValues: series.days.map { ($0.day, metric.value($0)) })

        // Keep a twelve-month calendar canvas like Codex's activity view so the
        // heatmap remains a readable timeline instead of collapsing into a few
        // isolated columns.
        let currentMonthStart = cal.date(
            from: cal.dateComponents([.year, .month], from: lastDate)
        ) ?? lastDate
        let annualStart = cal.date(byAdding: .month, value: -11, to: currentMonthStart) ?? firstDate
        let displayFirstDate = min(firstDate, annualStart)

        // Align to week boundaries (Monday-start).
        let weekdayOfFirst = (cal.component(.weekday, from: displayFirstDate) + 5) % 7
        let start = cal.date(byAdding: .day, value: -weekdayOfFirst, to: displayFirstDate) ?? displayFirstDate
        let weekdayOfLast = (cal.component(.weekday, from: lastDate) + 5) % 7
        let end = cal.date(byAdding: .day, value: 6 - weekdayOfLast, to: lastDate) ?? lastDate

        var weeks: [[CalendarDay?]] = []
        var current = start
        var week: [CalendarDay?] = []
        while current <= end && weeks.count < 60 {
            let key = fmt.string(from: current)
            let inRange = current >= displayFirstDate && current <= lastDate
            week.append(CalendarDay(date: key, value: values[key] ?? 0, inRange: inRange))
            if week.count == 7 {
                weeks.append(week)
                week = []
            }
            current = cal.date(byAdding: .day, value: 1, to: current) ?? current.addingTimeInterval(86_400)
        }
        if !week.isEmpty { weeks.append(week) }
        return weeks
    }

    private func monthLabels(weeks: [[CalendarDay?]], size: CGFloat, gap: CGFloat) -> [String] {
        var labels = Array(repeating: "", count: weeks.count)
        var previousMonth: Substring?
        var lastVisibleX = -CGFloat.infinity
        let stride = size + gap

        for (index, week) in weeks.enumerated() {
            guard let firstDate = week.compactMap({ $0 }).first(where: { $0.inRange })?.date else { continue }
            let parts = firstDate.split(separator: "-")
            guard parts.count == 3 else { continue }

            let month = parts[1]
            guard month != previousMonth else { continue }
            previousMonth = month

            let x = CGFloat(index) * stride
            guard x - lastVisibleX >= 24 else { continue }
            labels[index] = "\(month)月"
            lastVisibleX = x
        }
        return labels
    }

    // MARK: legend

    private var legend: some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) {
                Text("少")
                    .font(.system(size: 10))
                    .foregroundStyle(D.muted)
                ForEach(0..<6, id: \.self) { level in
                    RoundedRectangle(cornerRadius: 3, style: .continuous)
                        .fill(level == 0 ? D.cellEmpty : D.primary.opacity(Self.LEVEL_PCT[level] / 100))
                        .frame(width: 12, height: 12)
                        .overlay(
                            RoundedRectangle(cornerRadius: 3, style: .continuous)
                                .strokeBorder(D.hairline.opacity(0.6), lineWidth: 1)
                        )
                }
                Text("多")
                    .font(.system(size: 10))
                    .foregroundStyle(D.muted)
            }
            Spacer()
            HStack(spacing: 3) {
                Text("合计:")
                    .font(.system(size: 11))
                    .foregroundStyle(D.muted)
                Text(Formatters.full(rangeTotal))
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundStyle(D.foreground)
            }
        }
        .padding(.top, 4)
    }
}

// MARK: - Composition (BREAKDOWN, two half donuts)

struct CompositionCard: View {
    let series: UsageSeries

    private static let inputMeta: [(key: String, color: Color)] = [
        ("输入", Color(hex: 0x7AD7FF)),
        ("缓存读", Color(hex: 0x9E8CFF)),
        ("缓存写", Color(hex: 0xFF9E6E))
    ]
    private static let outputMeta: [(key: String, color: Color)] = [
        ("输出", Color(hex: 0xF3B56F)),
        ("推理", Color(hex: 0xFF8CC6))
    ]

    var body: some View {
        let cacheTotal = series.cacheReadTotal + series.cacheWriteTotal
        let inputTotal = series.inputTotal + cacheTotal
        let outputTotal = series.outputTotal
        let cachePct = inputTotal > 0 ? Double(cacheTotal) / Double(inputTotal) * 100 : 0
        let reasoningPct = outputTotal > 0 ? Double(series.reasoningTotal) / Double(outputTotal) * 100 : 0

        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Breakdown",
                    title: "Token 组成",
                    subtitle: "当前范围内各类 token 的占比"
                )

                HStack(alignment: .top, spacing: 8) {
                    VStack(spacing: 4) {
                        sideLabel("输入侧")
                        HalfDonut(
                            segments: [
                                .init(name: "输入", value: Double(series.inputTotal), color: Self.inputMeta[0].color),
                                .init(name: "缓存读", value: Double(series.cacheReadTotal), color: Self.inputMeta[1].color),
                                .init(name: "缓存写", value: Double(series.cacheWriteTotal), color: Self.inputMeta[2].color)
                            ],
                            centerLabel: "缓存占比",
                            centerValue: String(format: "%.1f%%", cachePct),
                            subLabel: Formatters.compact(cacheTotal)
                        )
                    }
                    .frame(maxWidth: .infinity)

                    Rectangle()
                        .fill(D.hairline.opacity(0.5))
                        .frame(width: 1)

                    VStack(spacing: 4) {
                        sideLabel("输出侧")
                        HalfDonut(
                            segments: [
                                .init(name: "输出", value: Double(series.outputTotal), color: Self.outputMeta[0].color),
                                .init(name: "推理", value: Double(series.reasoningTotal), color: Self.outputMeta[1].color)
                            ],
                            centerLabel: "推理占比",
                            centerValue: String(format: "%.1f%%", reasoningPct),
                            subLabel: Formatters.compact(series.reasoningTotal)
                        )
                    }
                    .frame(maxWidth: .infinity)
                }
            }
        }
    }

    private func sideLabel(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold))
            .tracking(1)
            .textCase(.uppercase)
            .foregroundStyle(D.muted)
            .frame(maxWidth: .infinity)
    }
}

struct HalfDonut: View {
    struct Segment: Identifiable {
        let id = UUID()
        let name: String
        let value: Double
        let color: Color
    }

    let segments: [Segment]
    let centerLabel: String
    let centerValue: String
    let subLabel: String

    var body: some View {
        VStack(spacing: 6) {
            ZStack(alignment: .bottom) {
                Canvas { context, size in
                    let total = segments.reduce(0) { $0 + $1.value }
                    guard total > 0 else { return }
                    let cx = size.width / 2
                    let cy = size.height / 2
                    let base = min(size.width / 2, size.height / 2)
                    let rOuter = base * 0.82
                    let rInner = base * 0.52
                    let padding: Double = segments.count > 1 ? Double.pi / 180 : 0

                    var start = Double.pi
                    for segment in segments where segment.value > 0 {
                        let sweep = segment.value / total * Double.pi
                        let a1 = start - padding
                        let a2 = start - sweep + padding
                        start -= sweep
                        guard a1 > a2 else { continue }

                        var path = Path()
                        let steps = 60
                        for i in 0...steps {
                            let t = a1 + (a2 - a1) * Double(i) / Double(steps)
                            let point = CGPoint(x: cx + rOuter * cos(t), y: cy - rOuter * sin(t))
                            if i == 0 { path.move(to: point) } else { path.addLine(to: point) }
                        }
                        for i in stride(from: steps, through: 0, by: -1) {
                            let t = a1 + (a2 - a1) * Double(i) / Double(steps)
                            path.addLine(to: CGPoint(x: cx + rInner * cos(t), y: cy - rInner * sin(t)))
                        }
                        path.closeSubpath()
                        context.fill(path, with: .color(segment.color))
                        context.stroke(path, with: .color(D.background), lineWidth: 2)
                    }
                }
                .frame(height: 140)

                VStack(spacing: 1) {
                    Text(centerLabel)
                        .font(.system(size: 10))
                        .foregroundStyle(D.muted)
                    Text(centerValue)
                        .font(.system(size: 16, weight: .bold, design: .monospaced))
                        .foregroundStyle(D.foreground)
                    Text(subLabel)
                        .font(.system(size: 10))
                        .foregroundStyle(D.muted)
                }
                .padding(.bottom, 4)
            }
            .frame(height: 140)

            HStack(spacing: 8) {
                ForEach(segments.filter { $0.value > 0 }) { segment in
                    HStack(spacing: 4) {
                        Circle()
                            .fill(segment.color)
                            .frame(width: 8, height: 8)
                        Text(segment.name)
                            .font(.system(size: 11))
                            .foregroundStyle(D.muted)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity)
    }
}

// MARK: - Trend (TREND, daily area chart)

struct TrendCard: View {
    let metric: DashMetric
    let series: UsageSeries
    // Parse days once per view value. The old computed property created a
    // DateFormatter and re-parsed every day on *each* access (including once
    // per point inside the Chart closure), which blocked the main thread for
    // many seconds while the chart laid out.
    private let points: [(date: Date, value: Double)]
    private let peak: (date: Date, value: Double)?

    init(metric: DashMetric, series: UsageSeries) {
        self.metric = metric
        self.series = series
        let parsed: [(date: Date, value: Double)] = series.days.compactMap { day in
            guard let date = Formatters.day.date(from: day.day) else { return nil }
            return (date, Double(metric.value(day)))
        }
        self.points = parsed
        self.peak = parsed.max(by: { $0.value < $1.value })
    }

    private var xStride: Int {
        let n = points.count
        if n <= 14 { return 1 }
        if n <= 30 { return 2 }
        if n <= 60 { return 3 }
        if n <= 90 { return 4 }
        return max(1, n / 30)
    }

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Trend",
                    title: "每日趋势",
                    subtitle: "按天展示 \(metric.label)"
                )

                if points.count > 1 {
                    Chart(points, id: \.date) { point in
                        AreaMark(
                            x: .value("日期", point.date, unit: .day),
                            y: .value("Tokens", point.value)
                        )
                        .foregroundStyle(
                            LinearGradient(
                                colors: [metric.color.opacity(0.35), metric.color.opacity(0.02)],
                                startPoint: .top,
                                endPoint: .bottom
                            )
                        )
                        .interpolationMethod(.monotone)

                        LineMark(
                            x: .value("日期", point.date, unit: .day),
                            y: .value("Tokens", point.value)
                        )
                        .foregroundStyle(metric.color)
                        .lineStyle(StrokeStyle(lineWidth: 2))
                        .interpolationMethod(.monotone)

                        if let peak,
                           peak.date == point.date {
                            PointMark(
                                x: .value("日期", point.date, unit: .day),
                                y: .value("Tokens", point.value)
                            )
                            .foregroundStyle(metric.color.opacity(0.85))
                            .symbolSize(64)
                        }
                    }
                    .chartXAxis {
                        AxisMarks(values: .stride(by: .day, count: xStride)) { _ in
                            AxisGridLine().foregroundStyle(Color.clear)
                            AxisTick().foregroundStyle(Color.clear)
                            AxisValueLabel(format: .dateTime.month(.abbreviated).day(), centered: true)
                                .font(.system(size: 9))
                                .foregroundStyle(D.muted)
                        }
                    }
                    .chartYAxis {
                        AxisMarks(position: .leading) { value in
                            AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [3]))
                                .foregroundStyle(D.hairline)
                            AxisValueLabel {
                                if let tokens = value.as(Double.self) {
                                    Text(Formatters.axis(Int64(tokens)))
                                        .font(.system(size: 9))
                                        .foregroundStyle(D.muted)
                                }
                            }
                        }
                    }
                    .chartLegend(.hidden)
                    .frame(height: 260)
                } else {
                    Text("暂无趋势数据，试试调整时间范围")
                        .font(.system(size: 13))
                        .foregroundStyle(D.muted)
                        .frame(maxWidth: .infinity, minHeight: 80)
                }

                HStack(spacing: 12) {
                    HStack(spacing: 5) {
                        RoundedRectangle(cornerRadius: 2)
                            .fill(metric.color)
                            .frame(width: 10, height: 10)
                        Text(metric.label)
                            .font(.system(size: 11))
                            .foregroundStyle(D.muted)
                    }
                    Spacer()
                }
            }
        }
    }
}

// MARK: - Models (LEADERBOARD)

struct ModelCard: View {
    let metric: DashMetric
    let models: [ModelUsageItem]
    @State private var expanded: String?

    private var topModels: [ModelUsageItem] {
        models
            .filter { metric.value($0) > 0 }
            .sorted { metric.value($0) > metric.value($1) }
            .prefix(8)
            .map { $0 }
    }

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Leaderboard",
                    title: "模型贡献",
                    subtitle: "按当前指标排序的 Top 8 模型"
                )

                if topModels.isEmpty {
                    Text("暂无模型数据")
                        .font(.system(size: 13))
                        .foregroundStyle(D.muted)
                        .frame(maxWidth: .infinity, minHeight: 80)
                } else {
                    let maxValue = metric.value(topModels[0])
                    VStack(spacing: 14) {
                        ForEach(topModels, id: \.id) { model in
                            ModelBarRow(
                                name: String(model.model.prefix(18)),
                                valueText: Formatters.axis(metric.value(model)),
                                ratio: maxValue > 0 ? Double(metric.value(model)) / Double(maxValue) : 0,
                                isExpanded: expanded == model.id.uuidString,
                                model: model
                            ) {
                                withAnimation(.easeInOut(duration: 0.15)) {
                                    if expanded == model.id.uuidString {
                                        expanded = nil
                                    } else {
                                        expanded = model.id.uuidString
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

struct ModelBarRow: View {
    let name: String
    let valueText: String
    let ratio: Double
    let isExpanded: Bool
    let model: ModelUsageItem
    let toggle: () -> Void

    var body: some View {
        VStack(spacing: 6) {
            Button(action: toggle) {
                HStack(spacing: 8) {
                    Text(name)
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(D.foreground)
                        .frame(width: 110, alignment: .leading)
                        .lineLimit(1)

                    GeometryReader { geo in
                        let barWidth = geo.size.width * min(1, max(0, ratio))
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: 4, style: .continuous)
                                .fill(D.muted.opacity(0.3))
                                .frame(height: 12)
                            UnevenRoundedRectangle(
                                topLeadingRadius: 0, bottomLeadingRadius: 0,
                                bottomTrailingRadius: 4, topTrailingRadius: 4, style: .continuous
                            )
                            .fill(
                                LinearGradient(
                                    colors: [D.chart2.opacity(0.75), D.chart1.opacity(0.95)],
                                    startPoint: .leading,
                                    endPoint: .trailing
                                )
                            )
                            .frame(width: max(2, barWidth), height: 12)
                            Text(valueText)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(D.muted)
                                .padding(.leading, barWidth + 6)
                        }
                    }
                    .frame(height: 14)
                    .padding(.trailing, 54)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityElement(children: .contain)
            .accessibilityLabel("\(name) \(valueText)")
            .help("点击展开 token 分解")

            if isExpanded {
                VStack(spacing: 5) {
                    tokenLine("输入", model.inputTokens, Color(hex: 0x7AD7FF))
                    tokenLine("输出", model.outputTokens, Color(hex: 0xF3B56F))
                    tokenLine("推理", model.reasoningTokens, Color(hex: 0xFF8CC6))
                    tokenLine("缓存读", model.cacheRead, Color(hex: 0x9E8CFF))
                    tokenLine("缓存写", model.cacheWrite, Color(hex: 0xFF9E6E))
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Color.white.opacity(0.04))
                )
                .padding(.leading, 118)
            }
        }
    }

    private func tokenLine(_ label: String, _ value: Int64, _ color: Color) -> some View {
        HStack {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(label)
                .font(.system(size: 11))
                .foregroundStyle(D.muted)
            Spacer()
            Text(Formatters.full(value))
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(D.foreground)
        }
    }
}

// MARK: - Providers (DISTRIBUTION, donut)

struct ProviderCard: View {
    let metric: DashMetric
    let providers: [ProviderUsageItem]

    private static let palette = [D.chart1, D.chart2, D.chart3, D.chart4, D.chart5]

    private struct Slice: Identifiable {
        let id = UUID()
        let name: String
        let value: Int64
        let color: Color
    }

    private var slices: [Slice] {
        let sorted = providers
            .filter { metric.value($0) > 0 }
            .sorted { metric.value($0) > metric.value($1) }
        let top = sorted.prefix(6).enumerated().map { index, item in
            Slice(name: item.provider, value: metric.value(item), color: Self.palette[index % Self.palette.count])
        }
        let rest = sorted.dropFirst(6).reduce(Int64(0)) { $0 + metric.value($1) }
        if rest > 0 {
            return top + [Slice(name: "其他", value: rest, color: D.muted)]
        }
        return top
    }

    private var total: Int64 { slices.reduce(0) { $0 + $1.value } }

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Distribution",
                    title: "Provider 分布",
                    subtitle: "当前指标在 provider 间的占比"
                )

                if slices.isEmpty {
                    Text("暂无 Provider 数据")
                        .font(.system(size: 13))
                        .foregroundStyle(D.muted)
                        .frame(maxWidth: .infinity, minHeight: 80)
                } else {
                    Canvas { context, size in
                        let cx = size.width / 2
                        let cy = size.height / 2
                        let base = min(size.width / 2, size.height / 2)
                        let rOuter = base * 0.92
                        let rInner = base * 0.55
                        guard total > 0 else { return }

                        var start = -Double.pi / 2
                        for slice in slices {
                            let sweep = Double(slice.value) / Double(total) * Double.pi * 2
                            let a1 = start + 0.015
                            let a2 = start + sweep - 0.015
                            start += sweep
                            guard a2 > a1 else { continue }

                            var path = Path()
                            let steps = 72
                            for i in 0...steps {
                                let t = a1 + (a2 - a1) * Double(i) / Double(steps)
                                let point = CGPoint(x: cx + rOuter * cos(t), y: cy + rOuter * sin(t))
                                if i == 0 { path.move(to: point) } else { path.addLine(to: point) }
                            }
                            for i in stride(from: steps, through: 0, by: -1) {
                                let t = a1 + (a2 - a1) * Double(i) / Double(steps)
                                path.addLine(to: CGPoint(x: cx + rInner * cos(t), y: cy + rInner * sin(t)))
                            }
                            path.closeSubpath()
                            context.fill(path, with: .color(slice.color))
                            context.stroke(path, with: .color(D.background), lineWidth: 2)
                        }
                    }
                    .frame(height: 170)

                    VStack(spacing: 7) {
                        ForEach(slices) { slice in
                            HStack(spacing: 7) {
                                Circle()
                                    .fill(slice.color)
                                    .frame(width: 8, height: 8)
                                Text(String(slice.name.prefix(9)))
                                    .font(.system(size: 11))
                                    .foregroundStyle(D.foreground)
                                    .lineLimit(1)
                                Spacer()
                                Text(total > 0 ? "\(Int(Double(slice.value) / Double(total) * 100))%" : "0%")
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(D.muted)
                                Text(Formatters.axis(slice.value))
                                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(D.foreground)
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: - Cache hit rate (PERFORMANCE)

struct CacheCard: View {
    let series: UsageSeries
    // Parse once per view value (see TrendCard for why this must not be a
    // computed property creating a DateFormatter per access).
    private let daily: [(date: Date, value: Double)]

    init(series: UsageSeries) {
        self.series = series
        self.daily = series.days.compactMap { day in
            guard day.inputSide > 0, let date = Formatters.day.date(from: day.day) else { return nil }
            return (date, series.hitRate(on: day))
        }
    }

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Performance",
                    title: "缓存命中率",
                    subtitle: "按天统计输入部分的缓存占比"
                )

                HStack(alignment: .top, spacing: 24) {
                    VStack(alignment: .leading, spacing: 12) {
                        Text(String(format: "%.1f%%", series.cacheHitRate))
                            .font(.system(size: 34, weight: .bold, design: .monospaced))
                            .foregroundStyle(D.foreground)

                        HStack(spacing: 20) {
                            statBlock("输入", series.inputTotal, Color(hex: 0x7AD7FF))
                            statBlock("缓存读", series.cacheReadTotal, Color(hex: 0x9E8CFF))
                            statBlock("缓存写", series.cacheWriteTotal, Color(hex: 0xFF9E6E))
                        }

                        Text("缓存读 ÷（输入 + 缓存读 + 缓存写）")
                            .font(.system(size: 11))
                            .foregroundStyle(D.muted)
                    }
                    .frame(width: 300, alignment: .leading)

                    if daily.count > 1 {
                        Chart(daily, id: \.date) { point in
                            AreaMark(
                                x: .value("日期", point.date, unit: .day),
                                y: .value("命中率", point.value)
                            )
                            .foregroundStyle(
                                LinearGradient(
                                    colors: [D.chart2.opacity(0.30), D.chart2.opacity(0.02)],
                                    startPoint: .top,
                                    endPoint: .bottom
                                )
                            )
                            .interpolationMethod(.monotone)

                            LineMark(
                                x: .value("日期", point.date, unit: .day),
                                y: .value("命中率", point.value)
                            )
                            .foregroundStyle(D.chart2)
                            .lineStyle(StrokeStyle(lineWidth: 2))
                            .interpolationMethod(.monotone)
                        }
                        .chartXAxis {
                            AxisMarks(values: .stride(by: .day, count: max(1, daily.count / 8))) { _ in
                                AxisGridLine().foregroundStyle(Color.clear)
                                AxisTick().foregroundStyle(Color.clear)
                                AxisValueLabel(format: .dateTime.month(.abbreviated).day(), centered: true)
                                    .font(.system(size: 9))
                                    .foregroundStyle(D.muted)
                            }
                        }
                        .chartYAxis {
                            AxisMarks(position: .leading) { value in
                                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [3]))
                                    .foregroundStyle(D.hairline)
                                AxisValueLabel {
                                    if let rate = value.as(Double.self) {
                                        Text("\(Int(rate))%")
                                            .font(.system(size: 9))
                                            .foregroundStyle(D.muted)
                                    }
                                }
                            }
                        }
                        .chartYScale(domain: 0...100)
                        .chartLegend(.hidden)
                        .frame(height: 190)
                    } else {
                        Text("暂无趋势数据，试试调整时间范围")
                            .font(.system(size: 13))
                            .foregroundStyle(D.muted)
                            .frame(maxWidth: .infinity, minHeight: 80)
                    }
                }
            }
        }
    }

    private func statBlock(_ title: String, _ value: Int64, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title)
                .font(.system(size: 11))
                .foregroundStyle(D.muted)
            Text(Formatters.compact(value))
                .font(.system(size: 14, weight: .semibold, design: .monospaced))
                .foregroundStyle(color)
        }
    }
}

// MARK: - Projects / tools (extra cards, same visual language)

struct ProjectsCard: View {
    let projects: [ProjectStats]

    private var rows: [(ProjectStats, Int64)] {
        projects
            .map { ($0, $0.inputTokens + $0.outputTokens + $0.cacheRead + $0.cacheWrite) }
            .sorted { $0.1 > $1.1 }
            .prefix(6)
            .map { $0 }
    }

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Projects",
                    title: "项目用量",
                    subtitle: "按总 Tokens 排序"
                )
                if rows.isEmpty {
                    Text("暂无项目数据")
                        .font(.system(size: 13))
                        .foregroundStyle(D.muted)
                        .frame(maxWidth: .infinity, minHeight: 60)
                } else {
                    let maxValue = Double(rows[0].1)
                    VStack(spacing: 10) {
                        ForEach(Array(rows.enumerated()), id: \.offset) { _, item in
                            SimpleBarRow(
                                title: item.0.name,
                                subtitle: "\(item.0.sessionCount) sessions",
                                value: Formatters.axis(item.1),
                                ratio: maxValue > 0 ? Double(item.1) / maxValue : 0,
                                color: D.chart5
                            )
                        }
                    }
                }
            }
        }
    }
}

struct ToolsCard: View {
    let tools: [ToolUsageItem]

    var body: some View {
        DashboardCard {
            VStack(alignment: .leading, spacing: 12) {
                DashboardSectionHeader(
                    eyebrow: "Tools",
                    title: "工具调用",
                    subtitle: "当前区间内调用次数 Top 8"
                )
                if tools.isEmpty {
                    Text("暂无工具数据")
                        .font(.system(size: 13))
                        .foregroundStyle(D.muted)
                        .frame(maxWidth: .infinity, minHeight: 60)
                } else {
                    let maxValue = Double(tools[0].count)
                    VStack(spacing: 10) {
                        ForEach(Array(tools.prefix(8).enumerated()), id: \.offset) { _, tool in
                            SimpleBarRow(
                                title: tool.name,
                                subtitle: "\(tool.count) 次",
                                value: "\(tool.count)",
                                ratio: maxValue > 0 ? Double(tool.count) / maxValue : 0,
                                color: D.chart2
                            )
                        }
                    }
                }
            }
        }
    }
}

struct SimpleBarRow: View {
    let title: String
    let subtitle: String
    let value: String
    let ratio: Double
    let color: Color

    var body: some View {
        VStack(spacing: 4) {
            HStack {
                Text(title)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(D.foreground)
                    .lineLimit(1)
                Spacer()
                Text(subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(D.muted)
                Text(value)
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(D.foreground)
                    .frame(width: 74, alignment: .trailing)
            }
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(D.muted.opacity(0.3))
                    Capsule()
                        .fill(color.opacity(0.85))
                        .frame(width: max(3, geo.size.width * min(1, max(0, ratio))))
                }
            }
            .frame(height: 6)
        }
    }
}
