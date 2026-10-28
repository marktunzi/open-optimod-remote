import AppKit
import Foundation
import WebKit

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuItemValidation, DeviceWindowBridgeDelegate {
    /// Meters, Processing Settings, System Settings and Presets: one native
    /// window each, all on the one processor session the service owns.
    private let windows = DeviceWindowManager()
    private var backend: Process?
    private var startedBackend = false
    private var loadedInterface = false
    private var terminationRequested = false
    private var terminationReplySent = false
    private var connectionsWindowController: ConnectionsWindowController?
    private var presetsWindowController: PresetsWindowController?
    private var errorLogWindowController: ErrorLogWindowController?
    private var presentedInitialConnections = false

    private lazy var session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 0.6
        configuration.timeoutIntervalForResource = 1.0
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: configuration)
    }()

    func applicationDidFinishLaunching(_ notification: Notification) {
        windows.appDelegate = self
        configureApplicationMenu()
        NSApp.activate(ignoringOtherApps: true)
        showStatus(title: "Starting Open Optimod Remote", detail: "Preparing the local control service…")
        startOrReuseBackend()
    }

    /// Closing every window keeps the processor connected; the app quits only
    /// with Quit, which ends the session cleanly.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag && loadedInterface { windows.show(.meters) }
        return true
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminationRequested else { return .terminateLater }
        terminationRequested = true
        windows.prepareForTermination()

        var request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/disconnect")!)
        request.httpMethod = "POST"
        request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
        session.dataTask(with: request) { [weak self] _, _, _ in
            DispatchQueue.main.async { self?.finishTermination() }
        }.resume()

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
            self?.finishTermination()
        }
        return .terminateLater
    }

    func applicationWillTerminate(_ notification: Notification) {
        stopOwnedBackend()
    }

    private func configureApplicationMenu() {
        let mainMenu = NSMenu()
        let applicationItem = NSMenuItem()
        let applicationMenu = NSMenu()
        let name = "Open Optimod Remote"

        applicationMenu.addItem(
            withTitle: "About \(name)",
            action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)),
            keyEquivalent: ""
        )
        applicationMenu.addItem(.separator())
        let connectionsItem = applicationMenu.addItem(
            withTitle: "Connections…",
            action: #selector(showConnections(_:)),
            keyEquivalent: "k"
        )
        connectionsItem.target = self
        let settingsItem = applicationMenu.addItem(
            withTitle: "System Settings…",
            action: #selector(showSystemSettings(_:)),
            keyEquivalent: ","
        )
        settingsItem.target = self
        let presetsItem = applicationMenu.addItem(
            withTitle: "Presets…",
            action: #selector(showPresets(_:)),
            keyEquivalent: "p"
        )
        presetsItem.keyEquivalentModifierMask = [.command, .shift]
        presetsItem.target = self
        let errorsItem = applicationMenu.addItem(
            withTitle: "Error Log…",
            action: #selector(showErrorLog(_:)),
            keyEquivalent: "e"
        )
        errorsItem.keyEquivalentModifierMask = [.command, .shift]
        errorsItem.target = self
        applicationMenu.addItem(.separator())
        applicationMenu.addItem(
            withTitle: "Quit \(name)",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        applicationItem.submenu = applicationMenu
        mainMenu.addItem(applicationItem)

        let windowItem = NSMenuItem()
        let windowMenu = NSMenu(title: "Window")
        for (kind, key) in [(DeviceWindowKind.meters, "1"), (.processing, "2")] {
            let item = windowMenu.addItem(withTitle: kind.title, action: #selector(showDeviceWindow(_:)), keyEquivalent: key)
            item.representedObject = kind.rawValue
            item.target = self
        }
        windowMenu.addItem(.separator())
        windowMenu.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        windowMenu.addItem(withTitle: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        windowMenu.addItem(.separator())
        windowMenu.addItem(withTitle: "Bring All to Front", action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        windowItem.submenu = windowMenu
        mainMenu.addItem(windowItem)
        NSApp.mainMenu = mainMenu
        NSApp.windowsMenu = windowMenu
    }

    private func showStatus(title: String, detail: String) {
        let html = """
        <!doctype html><html><head><meta charset="utf-8"><style>
        :root { color-scheme: dark; font-family: -apple-system, BlinkMacSystemFont, sans-serif; }
        body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #292a2b; color: #f0f0f0; }
        main { width: min(520px, 80vw); padding: 34px; background: #343536; border: 1px solid #666; border-radius: 10px; }
        h1 { margin: 0 0 10px; font-size: 22px; } p { margin: 0; color: #c4c4c4; line-height: 1.5; }
        .meter { height: 4px; margin-top: 26px; overflow: hidden; background: #1b1b1c; }
        .meter::after { content: ''; display: block; width: 42%; height: 100%; background: #38c6e8; animation: move 1.1s ease-in-out infinite alternate; }
        @keyframes move { from { transform: translateX(-90%); } to { transform: translateX(220%); } }
        @media (prefers-reduced-motion: reduce) { .meter::after { animation: none; transform: none; } }
        </style></head><body><main><h1>\(title)</h1><p>\(detail)</p><div class="meter"></div></main></body></html>
        """
        windows.showStartup(html: html)
    }

    private func startOrReuseBackend() {
        probeBackend { [weak self] available in
            guard let self else { return }
            if available {
                self.loadInterface()
            } else {
                self.startBundledBackend()
            }
        }
    }

    private func probeBackend(completion: @escaping (Bool) -> Void) {
        var request = URLRequest(url: BackendHealth.endpoint)
        request.cachePolicy = .reloadIgnoringLocalCacheData
        session.dataTask(with: request) { data, response, _ in
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            let valid = BackendHealth.isOwnService(statusCode: status, data: data)
            DispatchQueue.main.async { completion(valid) }
        }.resume()
    }

    private func startBundledBackend() {
        let executable = Bundle.main.resourceURL?
            .appendingPathComponent("backend", isDirectory: true)
            .appendingPathComponent("orban-web", isDirectory: false)
        guard let executable,
              FileManager.default.isExecutableFile(atPath: executable.path)
        else {
            showStartupError("The bundled control service is missing.")
            return
        }

        let process = Process()
        process.executableURL = executable
        process.currentDirectoryURL = Bundle.main.resourceURL
        var environment = ProcessInfo.processInfo.environment
        environment["OPEN_OPTIMOD_NO_BROWSER"] = "1"
        process.environment = environment
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            backend = process
            startedBackend = true
            waitForBackend(attempt: 0)
        } catch {
            showStartupError("The local control service could not start: \(error.localizedDescription)")
        }
    }

    private func waitForBackend(attempt: Int) {
        probeBackend { [weak self] available in
            guard let self else { return }
            if available {
                self.loadInterface()
                return
            }
            if attempt >= 79 || self.backend?.isRunning == false {
                self.showStartupError("The local control service did not become ready.")
                return
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                self.waitForBackend(attempt: attempt + 1)
            }
        }
    }

    private func loadInterface() {
        guard !loadedInterface else { return }
        loadedInterface = true
        windows.interfaceDidLoad()
        startWindowControl()
        if !presentedInitialConnections {
            presentedInitialConnections = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self] in
                self?.showConnections(nil)
            }
        }
    }

    private func showStartupError(_ message: String) {
        showStatus(title: "Open Optimod Remote could not start", detail: message)
        let alert = NSAlert()
        alert.alertStyle = .critical
        alert.messageText = "Open Optimod Remote could not start"
        alert.informativeText = message
        alert.addButton(withTitle: "Retry")
        alert.addButton(withTitle: "Quit")
        if alert.runModal() == .alertFirstButtonReturn {
            loadedInterface = false
            stopOwnedBackend()
            backend = nil
            startedBackend = false
            showStatus(title: "Starting Open Optimod Remote", detail: "Retrying the local control service…")
            startOrReuseBackend()
        } else {
            NSApp.terminate(nil)
        }
    }

    private func finishTermination() {
        guard terminationRequested, !terminationReplySent else { return }
        terminationReplySent = true
        session.invalidateAndCancel()
        stopOwnedBackend()
        NSApp.reply(toApplicationShouldTerminate: true)
    }

    private var windowControlOffset: UInt64 = 0
    private var windowControlTimer: Timer?

    /// Test hook: OPEN_OPTIMOD_WINDOW_CONTROL names a file of window actions.
    private func startWindowControl() {
        guard windowControlTimer == nil,
              let path = ProcessInfo.processInfo.environment["OPEN_OPTIMOD_WINDOW_CONTROL"]
        else { return }
        FileManager.default.createFile(atPath: path, contents: nil)
        windowControlTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            guard let self, let handle = FileHandle(forReadingAtPath: path) else { return }
            defer { try? handle.close() }
            try? handle.seek(toOffset: self.windowControlOffset)
            let data = handle.readDataToEndOfFile()
            guard let text = String(data: data, encoding: .utf8), let end = text.lastIndex(of: "\n") else { return }
            let complete = text[...end]
            self.windowControlOffset += UInt64(complete.utf8.count)
            for line in complete.split(separator: "\n") {
                self.windows.perform(control: line.trimmingCharacters(in: .whitespaces))
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                let state = try? JSONSerialization.data(withJSONObject: self.windows.windowList, options: [.prettyPrinted, .sortedKeys])
                FileManager.default.createFile(atPath: path + ".state", contents: state)
            }
        }
    }

    /// Command-, opens the System Settings of the connected processor's model.
    @objc private func showSystemSettings(_ sender: Any?) {
        guard loadedInterface else { return }
        windows.showSystemSettings()
    }

    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(showSystemSettings(_:)) {
            return loadedInterface && DeviceSessionStore.shared.device != nil
        }
        return true
    }

    @objc private func showDeviceWindow(_ sender: NSMenuItem) {
        guard loadedInterface,
              let raw = sender.representedObject as? String,
              let kind = DeviceWindowKind(rawValue: raw)
        else { return }
        windows.show(kind)
    }

    @objc private func showConnections(_ sender: Any?) {
        if connectionsWindowController == nil {
            let controller = ConnectionsWindowController()
            controller.onConnected = { [weak self] in
                self?.windows.deviceConnected()
            }
            connectionsWindowController = controller
        }
        connectionsWindowController?.show()
    }

    /// Shift-Command-P opens the Presets window, which has every preset
    /// function for the connected model. Before the interface has loaded the
    /// native preset window opens.
    @objc private func showPresets(_ sender: Any?) {
        if loadedInterface { windows.show(.presets) } else { showNativePresets() }
    }

    private func showNativePresets() {
        if presetsWindowController == nil {
            let controller = PresetsWindowController()
            controller.onChanged = { [weak self] in
                self?.connectionsWindowController?.refreshStatusNow()
            }
            presetsWindowController = controller
        }
        presetsWindowController?.show()
    }

    @objc private func showErrorLog(_ sender: Any?) {
        if errorLogWindowController == nil { errorLogWindowController = ErrorLogWindowController() }
        errorLogWindowController?.show()
    }

    func deviceWindow(_ webView: WKWebView?, didRequest name: String, body: Any) {
        switch name {
        case "openConnections": showConnections(nil)
        case "recordError":
            guard let body = body as? [String: Any], let text = body["message"] as? String else { break }
            ErrorLogStore.shared.record(source: body["source"] as? String ?? "Interface", message: text)
        default: break
        }
    }

    private func stopOwnedBackend() {
        guard startedBackend, let backend else { return }
        let processIdentifier = backend.processIdentifier
        if backend.isRunning { backend.terminate() }

        let pidFile = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Caches/OpenOptimodRemote/service.pid")
        let recordedPID = try? String(contentsOf: pidFile, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if recordedPID == String(processIdentifier) {
            try? FileManager.default.removeItem(at: pidFile)
        }
    }
}

@main
enum OpenOptimodApplication {
    static func main() {
        let application = NSApplication.shared
        application.setActivationPolicy(.regular)
        let delegate = AppDelegate()
        application.delegate = delegate
        withExtendedLifetime(delegate) {
            application.run()
        }
    }
}
