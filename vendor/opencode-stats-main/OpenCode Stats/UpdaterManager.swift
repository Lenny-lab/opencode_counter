//
//  UpdaterManager.swift
//  OpenCode Stats
//
//  Local build: Sparkle auto-updater removed (this fork carries local
//  modifications, so updating from upstream would overwrite them).
//

import AppKit
import Combine
import Foundation

@MainActor
final class UpdaterManager: NSObject, ObservableObject {
    @Published var canCheckForUpdates = false

    var automaticallyChecksForUpdates: Bool {
        get { false }
        set { }
    }

    override init() {
        super.init()
    }

    func start() {
        // Auto-update disabled in the local build.
    }

    func checkForUpdates() {
        let alert = NSAlert()
        alert.messageText = "Local build — auto-update disabled"
        alert.informativeText = "This is a locally modified build of OpenCode Stats. Updating from the upstream release would overwrite the local changes (total/token metrics)."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }
}
