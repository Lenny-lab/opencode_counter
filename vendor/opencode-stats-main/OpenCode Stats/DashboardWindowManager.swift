//
//  DashboardWindowManager.swift
//  OpenCode Stats
//
//  Hosts the full-size native dashboard window (no browser, no local server).
//

import AppKit
import SwiftUI

/// Keeps the Dock/Cmd-Tab presence in sync with which windows are open:
/// any visible window → normal app (`.regular`), otherwise menu-bar only.
enum AppActivationPolicy {
    static func sync() {
        let windowVisible = DashboardWindowManager.shared.isVisible
            || SettingsWindowManager.shared.isVisible
        NSApp.setActivationPolicy(windowVisible ? .regular : .accessory)
    }
}

@MainActor
final class DashboardWindowManager: NSObject, NSWindowDelegate {
    static let shared = DashboardWindowManager()

    private var windowController: NSWindowController?
    private var autoRefreshTimer: Timer?

    private override init() {
        super.init()
    }

    var isVisible: Bool {
        windowController?.window?.isVisible ?? false
    }

    private func makeWindowController() -> NSWindowController {
        let hostingController = NSHostingController(rootView: DashboardView())
        let window = NSWindow(contentViewController: hostingController)
        window.title = "OpenCode 用量看板"
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.titlebarSeparatorStyle = .line
        window.backgroundColor = NSColor(
            srgbRed: 0x04 / 255.0,
            green: 0x06 / 255.0,
            blue: 0x0D / 255.0,
            alpha: 1
        )
        window.isReleasedWhenClosed = false
        window.setContentSize(NSSize(width: 1180, height: 800))
        window.minSize = NSSize(width: 1000, height: 660)
        window.center()
        window.collectionBehavior = [.moveToActiveSpace]
        window.delegate = self
        return NSWindowController(window: window)
    }

    func show() {
        if windowController == nil {
            windowController = makeWindowController()
        }
        guard let window = windowController?.window else { return }

        windowController?.showWindow(nil)
        window.makeKeyAndOrderFront(nil)
        AppActivationPolicy.sync()
        NSApp.activate(ignoringOtherApps: true)

        if OpenCodeDatabase.shared.dbExists {
            OpenCodeDatabase.shared.daysFilter = DashboardDateRange.dayCount
            OpenCodeDatabase.shared.refresh()
        }
        startAutoRefresh()
    }

    func close() {
        windowController?.window?.close()
    }

    /// Refresh the dashboard data periodically while the window is on screen,
    /// so heatmap / cards / charts stay in sync during active sessions.
    private func startAutoRefresh() {
        autoRefreshTimer?.invalidate()
        let timer = Timer(timeInterval: OpenCodeDatabase.refreshInterval, repeats: true) { [weak self] _ in
            guard let self, self.isVisible else { return }
            if OpenCodeDatabase.shared.dbExists, !OpenCodeDatabase.shared.isLoading {
                OpenCodeDatabase.shared.daysFilter = DashboardDateRange.dayCount
                OpenCodeDatabase.shared.refresh(backgroundTriggered: true)
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        autoRefreshTimer = timer
    }

    private func stopAutoRefresh() {
        autoRefreshTimer?.invalidate()
        autoRefreshTimer = nil
    }

    func windowWillClose(_ notification: Notification) {
        stopAutoRefresh()
        AppActivationPolicy.sync()
    }
}
