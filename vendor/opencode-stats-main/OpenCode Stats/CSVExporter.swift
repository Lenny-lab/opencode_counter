//
//  CSVExporter.swift
//  OpenCode Stats
//
//  Builds a CSV data table of daily usage for a selected date range:
//  one row per day with input / output / reasoning / cache tokens,
//  total tokens and cost.
//

import Foundation
import SQLite3

// MARK: - Selection

enum ExportSelection {
    /// Whole history (min → max message time).
    case all
    /// Inclusive day range (start-of-day of `start` → start-of-day of `end`).
    case range(start: Date, end: Date)

    var isAll: Bool {
        if case .all = self { return true }
        return false
    }
}

enum ExportError: LocalizedError {
    case cannotOpen(String)
    case queryFailed(String)

    var errorDescription: String? {
        switch self {
        case .cannotOpen(let path): "无法打开数据库: \(path)"
        case .queryFailed(let message): "查询失败: \(message)"
        }
    }
}

enum CSVExporter {
    struct Result {
        let csv: String
        let fileName: String
        let startDay: String
        let endDay: String
        let dayCount: Int
    }

    static func export(dbPath: String, selection: ExportSelection) throws -> Result {
        var db: OpaquePointer? = nil
        guard sqlite3_open_v2(dbPath, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK, let db else {
            throw ExportError.cannotOpen(dbPath)
        }
        defer { sqlite3_close(db) }

        let cal = Calendar.current
        let dayFmt = DateFormatter()
        dayFmt.dateFormat = "yyyy-MM-dd"

        let startA: Int64
        let endB: Int64
        let startDay: String
        let endDay: String
        switch selection {
        case .all:
            let row = try query(db, "SELECT MIN(time_created), MAX(time_created) FROM message")
            let minTs = row.first.flatMap { i64($0[0]) } ?? 0
            let maxTs = row.first.flatMap { i64($0[1]) } ?? 0
            startA = minTs > 0 ? minTs : 0
            endB = maxTs > 0 ? maxTs + 1 : Int64(Date().timeIntervalSince1970 * 1000) + 1
            startDay = minTs > 0 ? dayFmt.string(from: Date(timeIntervalSince1970: Double(minTs) / 1000)) : "—"
            endDay = maxTs > 0 ? dayFmt.string(from: Date(timeIntervalSince1970: Double(maxTs) / 1000)) : "—"
        case .range(let start, let end):
            let s = cal.startOfDay(for: min(start, end))
            let e0 = cal.startOfDay(for: max(start, end))
            let e1 = cal.date(byAdding: .day, value: 1, to: e0) ?? e0.addingTimeInterval(86_400)
            startA = Int64(s.timeIntervalSince1970 * 1000)
            endB = Int64(e1.timeIntervalSince1970 * 1000)
            startDay = dayFmt.string(from: s)
            endDay = dayFmt.string(from: e0)
        }

        let win = "m.time_created >= \(startA) AND m.time_created < \(endB)"

        func partSum(_ key: String) -> String {
            """
            SUM(CASE WHEN json_extract(m.data, '$.role') = 'assistant'
                     THEN COALESCE(json_extract(m.data, '$.tokens.\(key)'), 0)
                     ELSE 0 END)
            """
        }
        let daySQL = """
            SELECT date(m.time_created/1000, 'unixepoch', 'localtime') as day,
                   \(partSum("input")),
                   \(partSum("output")),
                   \(partSum("reasoning")),
                   \(partSum("cache.read")),
                   \(partSum("cache.write")),
                   SUM(CASE WHEN json_extract(m.data, '$.role') = 'assistant'
                            THEN COALESCE(json_extract(m.data, '$.cost'), 0) ELSE 0 END)
            FROM message m
            WHERE \(win)
            GROUP BY day
            ORDER BY day
        """

        struct DayRow {
            var input: Int64 = 0
            var output: Int64 = 0
            var reasoning: Int64 = 0
            var cacheRead: Int64 = 0
            var cacheWrite: Int64 = 0
            var cost: Double = 0

            var total: Int64 { input + output + reasoning + cacheRead + cacheWrite }
        }

        var rowsByDay: [String: DayRow] = [:]
        for row in try query(db, daySQL) {
            var r = DayRow()
            r.input = i64(row[1])
            r.output = i64(row[2])
            r.reasoning = i64(row[3])
            r.cacheRead = i64(row[4])
            r.cacheWrite = i64(row[5])
            r.cost = dbl(row[6])
            rowsByDay[str(row[0])] = r
        }

        // Every day in the window (zero-filled) so the table has no gaps.
        var days: [String] = []
        if let first = dayFmt.date(from: startDay), let last = dayFmt.date(from: endDay) {
            var cur = first
            var guardCount = 0
            while cur <= last && guardCount < 10_000 {
                days.append(dayFmt.string(from: cur))
                cur = cal.date(byAdding: .day, value: 1, to: cur) ?? cur.addingTimeInterval(86_400)
                guardCount += 1
            }
        } else {
            days = rowsByDay.keys.sorted()
        }

        var lines: [String] = ["日期,输入,输出,推理,缓存读,缓存写,总Token,成本(USD)"]
        for day in days {
            let r = rowsByDay[day] ?? DayRow()
            lines.append([
                day,
                String(r.input),
                String(r.output),
                String(r.reasoning),
                String(r.cacheRead),
                String(r.cacheWrite),
                String(r.total),
                String(format: "%.4f", r.cost),
            ].joined(separator: ","))
        }
        // UTF-8 BOM so Excel / WPS detect the encoding.
        let csv = "\u{FEFF}" + lines.joined(separator: "\r\n") + "\r\n"

        return Result(
            csv: csv,
            fileName: "opencode-usage_\(startDay)_\(endDay).csv",
            startDay: startDay,
            endDay: endDay,
            dayCount: days.count
        )
    }
}

// MARK: - Query helpers

private func query(_ db: OpaquePointer, _ sql: String) throws -> [[Any]] {
    var stmt: OpaquePointer? = nil
    defer { sqlite3_finalize(stmt) }
    guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK, let stmt else {
        throw ExportError.queryFailed(String(cString: sqlite3_errmsg(db)))
    }
    var rows: [[Any]] = []
    while sqlite3_step(stmt) == SQLITE_ROW {
        let count = sqlite3_column_count(stmt)
        var row: [Any] = []
        for i in 0..<count {
            switch sqlite3_column_type(stmt, i) {
            case SQLITE_INTEGER: row.append(Int64(sqlite3_column_int64(stmt, i)))
            case SQLITE_FLOAT: row.append(Double(sqlite3_column_double(stmt, i)))
            case SQLITE_TEXT:
                if let c = sqlite3_column_text(stmt, i) { row.append(String(cString: c)) } else { row.append("") }
            default: row.append(NSNull())
            }
        }
        rows.append(row)
    }
    return rows
}

private func i64(_ value: Any?) -> Int64 {
    switch value {
    case let v as Int64: v
    case let v as Int: Int64(v)
    case let v as Double: Int64(v)
    case let v as String: Int64(v) ?? 0
    default: 0
    }
}

private func dbl(_ value: Any?) -> Double {
    switch value {
    case let v as Double: v
    case let v as Int64: Double(v)
    case let v as Int: Double(v)
    default: 0
    }
}

private func str(_ value: Any?) -> String {
    if let v = value as? String { return v }
    return ""
}
