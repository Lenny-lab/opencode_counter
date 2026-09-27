import SwiftUI
import AppKit

struct ExportSheet: View {
    @ObservedObject private var db = OpenCodeDatabase.shared
    @Environment(\.dismiss) private var dismiss

    enum Preset: String, CaseIterable, Identifiable {
        case today = "今天"
        case yesterday = "昨天"
        case last7 = "近 7 天"
        case last30 = "近 30 天"
        case all = "全部"
        case custom = "自定义"
        var id: String { rawValue }
    }

    enum Phase {
        case idle
        case running
        case done(fileName: String, url: URL)
        case failed(message: String)
    }

    @State private var preset: Preset = .last7
    @State private var customStart: Date = Calendar.current.date(byAdding: .day, value: -6, to: Date()) ?? Date()
    @State private var customEnd: Date = Date()
    @State private var phase: Phase = .idle

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            header

            presetRow

            if preset == .custom {
                customRow
            }

            previewRow

            statusBox

            Spacer(minLength: 0)

            buttonsRow
        }
        .padding(20)
        .frame(width: 560, height: 420, alignment: .topLeading)
        .background(D.background)
        .preferredColorScheme(.dark)
    }

    // MARK: Header

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("EXPORT · CSV")
                .font(.system(size: 10, weight: .semibold))
                .tracking(1.4)
                .textCase(.uppercase)
                .foregroundStyle(D.chart2)
            Text("导出每日用量数据表")
                .font(.system(size: 22, weight: .bold))
                .foregroundStyle(D.foreground)
            Text("生成 CSV 数据表（每天一行：输入/输出/推理/缓存 Token、总 Token、成本），可直接用表格软件打开。导出到「下载」文件夹。")
                .font(.system(size: 12))
                .foregroundStyle(D.muted)
        }
    }

    // MARK: Presets

    private var presetRow: some View {
        HStack(spacing: 6) {
            ForEach(Preset.allCases) { option in
                let selected = preset == option
                Button {
                    preset = option
                    if case .failed = phase { phase = .idle }
                    if case .done = phase { phase = .idle }
                } label: {
                    Text(option.rawValue)
                        .font(.system(size: 12, weight: selected ? .medium : .regular))
                        .foregroundStyle(selected ? D.primary : D.foreground.opacity(0.8))
                        .padding(.horizontal, 10)
                        .frame(height: 28)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(selected ? D.primary.opacity(0.15) : Color.clear)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .strokeBorder(
                                    selected ? D.primary.opacity(0.30) : D.subtleBorder,
                                    lineWidth: 1
                                )
                        )
                }
                .buttonStyle(.plain)
                .disabled(isBusy)
                .accessibilityLabel("导出范围 \(option.rawValue)")
            }
        }
    }

    private var customRow: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text("开始")
                    .font(.system(size: 11))
                    .foregroundStyle(D.muted)
                DatePicker("", selection: $customStart, in: ...Date(), displayedComponents: .date)
                    .datePickerStyle(.compact)
                    .labelsHidden()
                    .accessibilityLabel("开始日期")
            }
            VStack(alignment: .leading, spacing: 4) {
                Text("结束")
                    .font(.system(size: 11))
                    .foregroundStyle(D.muted)
                DatePicker("", selection: $customEnd, in: ...Date(), displayedComponents: .date)
                    .datePickerStyle(.compact)
                    .labelsHidden()
                    .accessibilityLabel("结束日期")
            }
            Spacer(minLength: 0)
        }
    }

    private var previewRow: some View {
        let info = rangeInfo()
        return HStack(spacing: 8) {
            Image(systemName: "calendar")
                .font(.system(size: 11))
                .foregroundStyle(D.chart3)
            Text("区间")
                .font(.system(size: 11))
                .foregroundStyle(D.muted)
            Text(info.label)
                .font(.system(size: 13, weight: .semibold, design: .monospaced))
                .foregroundStyle(D.foreground)
            Text("· \(info.days) 天")
                .font(.system(size: 12))
                .foregroundStyle(D.muted)
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(D.secondaryFill.opacity(0.6))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(D.panelBorder, lineWidth: 1)
        )
        .accessibilityLabel("导出区间 \(info.label)，共 \(info.days) 天")
    }

    // MARK: Status

    @ViewBuilder
    private var statusBox: some View {
        switch phase {
        case .idle:
            statusLine(icon: "square.and.arrow.down", color: D.muted, text: "选择范围后点击「导出 CSV」。")

        case .running:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("正在生成 CSV…")
                    .font(.system(size: 12))
                    .foregroundStyle(D.foreground)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(D.secondaryFill.opacity(0.6))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(D.panelBorder, lineWidth: 1)
            )
            .accessibilityLabel("正在生成 CSV")

        case .done(let fileName, let url):
            VStack(alignment: .leading, spacing: 8) {
                statusLine(icon: "checkmark.circle.fill", color: D.chart2,
                           text: "已导出 \(fileName)")
                HStack(spacing: 8) {
                    smallAction("打开", systemImage: "eye") {
                        NSWorkspace.shared.open(url)
                    }
                    smallAction("在 Finder 中显示", systemImage: "folder") {
                        NSWorkspace.shared.activateFileViewerSelecting([url])
                    }
                }
            }
            .accessibilityLabel("导出成功 \(fileName)")

        case .failed(let message):
            statusLine(icon: "exclamationmark.triangle.fill", color: Color(red: 0.95, green: 0.42, blue: 0.36),
                       text: "导出失败：\(message)")
        }
    }

    private func statusLine(icon: String, color: Color, text: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .font(.system(size: 12))
                .foregroundStyle(color)
            Text(text)
                .font(.system(size: 12))
                .foregroundStyle(D.foreground)
                .lineLimit(2)
                .minimumScaleFactor(0.8)
            Spacer(minLength: 0)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(D.secondaryFill.opacity(0.6))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(D.panelBorder, lineWidth: 1)
        )
    }

    private func smallAction(_ label: String, systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 5) {
                Image(systemName: systemImage)
                    .font(.system(size: 10))
                Text(label)
                    .font(.system(size: 11))
            }
            .foregroundStyle(D.foreground)
            .padding(.horizontal, 10)
            .frame(height: 26)
            .background(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color.black.opacity(0.25))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .strokeBorder(D.subtleBorder, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    // MARK: Buttons

    private var buttonsRow: some View {
        HStack(spacing: 10) {
            Spacer(minLength: 0)
            Button {
                dismiss()
            } label: {
                Text("关闭")
                    .font(.system(size: 13))
                    .foregroundStyle(D.foreground)
                    .padding(.horizontal, 16)
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
            .keyboardShortcut(.cancelAction)
            .accessibilityLabel("关闭导出窗口")

            Button {
                startExport()
            } label: {
                HStack(spacing: 6) {
                    if isBusy {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: "square.and.arrow.down")
                            .font(.system(size: 11))
                    }
                    Text(isBusy ? "导出中…" : "导出 CSV")
                        .font(.system(size: 13, weight: .medium))
                }
                .foregroundStyle(.white)
                .padding(.horizontal, 16)
                .frame(height: 32)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(D.primary.opacity(isBusy ? 0.5 : 0.9))
                )
            }
            .buttonStyle(.plain)
            .disabled(isBusy)
            .accessibilityLabel("导出 CSV")
        }
    }

    // MARK: Logic

    private var isBusy: Bool {
        if case .running = phase { return true }
        return false
    }

    private func resolvedSelection() -> ExportSelection {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        switch preset {
        case .today:
            return .range(start: today, end: today)
        case .yesterday:
            let d = cal.date(byAdding: .day, value: -1, to: today) ?? today
            return .range(start: d, end: d)
        case .last7:
            let s = cal.date(byAdding: .day, value: -6, to: today) ?? today
            return .range(start: s, end: today)
        case .last30:
            let s = cal.date(byAdding: .day, value: -29, to: today) ?? today
            return .range(start: s, end: today)
        case .all:
            return .all
        case .custom:
            return .range(start: customStart, end: customEnd)
        }
    }

    private func rangeInfo() -> (label: String, days: Int) {
        let cal = Calendar.current
        let dayFmt = DateFormatter()
        dayFmt.dateFormat = "yyyy/MM/dd"
        switch resolvedSelection() {
        case .all:
            return ("全部历史数据", 0)
        case .range(let s, let e):
            let a = cal.startOfDay(for: min(s, e))
            let b = cal.startOfDay(for: max(s, e))
            let days = (cal.dateComponents([.day], from: a, to: b).day ?? 0) + 1
            return ("\(dayFmt.string(from: a)) — \(dayFmt.string(from: b))", days)
        }
    }

    private func startExport() {
        guard !isBusy else { return }
        let selection = resolvedSelection()
        let dbPath = db.dbPath
        phase = .running

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let r = try ExportSheet.runExport(dbPath: dbPath, selection: selection)
                DispatchQueue.main.async {
                    phase = .done(fileName: r.fileName, url: r.url)
                }
            } catch {
                DispatchQueue.main.async {
                    phase = .failed(message: error.localizedDescription)
                }
            }
        }
    }

    /// Shared export path used by the sheet UI and by the `-AutoExport`
    /// launch hook (headless end-to-end verification).
    nonisolated static func runExport(dbPath: String, selection: ExportSelection) throws -> (url: URL, fileName: String, dayCount: Int) {
        let result = try CSVExporter.export(dbPath: dbPath, selection: selection)
        let downloads = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Downloads", isDirectory: true)
        try? FileManager.default.createDirectory(at: downloads, withIntermediateDirectories: true)
        let url = downloads.appendingPathComponent(result.fileName)
        guard let data = result.csv.data(using: .utf8) else {
            throw ExportError.cannotOpen(result.fileName)
        }
        try data.write(to: url, options: .atomic)
        return (url, result.fileName, result.dayCount)
    }
}
