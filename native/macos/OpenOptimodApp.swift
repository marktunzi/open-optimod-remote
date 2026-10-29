import AppKit
import Foundation
import UniformTypeIdentifiers
import WebKit

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuItemValidation, DeviceWindowBridgeDelegate {
    /// Meters and Processing Settings show the selected processor tab;
    /// System Settings, Presets and the Tools windows are one per processor.
    /// Each processor has its own session in the service.
    private let windows = DeviceWindowManager()
    private var backend: Process?
    private var startedBackend = false
    private var loadedInterface = false
    private var terminationRequested = false
    private var terminationReplySent = false
    private var connectionsWindowController: ConnectionsWindowController?
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
        windows.onPresetsChanged = { [weak self] in self?.connectionsWindowController?.refreshStatusNow() }
        windows.onDisconnect = { [weak self] id in self?.disconnect(id) }
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

        // Every processor session ends, each on its own.
        var request = URLRequest(url: BackendHealth.api("disconnect-all"))
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
        let updatesItem = applicationMenu.addItem(
            withTitle: "Check for Updates…",
            action: #selector(AppUpdater.checkForUpdates(_:)),
            keyEquivalent: ""
        )
        updatesItem.target = AppUpdater.shared
        let installItem = applicationMenu.addItem(
            withTitle: "Install Update…",
            action: #selector(AppUpdater.installPendingUpdate(_:)),
            keyEquivalent: ""
        )
        installItem.target = AppUpdater.shared
        installItem.isHidden = true
        applicationMenu.delegate = AppUpdater.shared
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
        let openSetupItem = applicationMenu.addItem(withTitle: "Open Setup…", action: #selector(openSetup(_:)), keyEquivalent: "")
        openSetupItem.target = self
        let saveSetupItem = applicationMenu.addItem(withTitle: "Save Setup As…", action: #selector(saveSetupAs(_:)), keyEquivalent: "")
        saveSetupItem.target = self
        let errorsItem = applicationMenu.addItem(
            withTitle: "Error Log…",
            action: #selector(showErrorLog(_:)),
            keyEquivalent: "e"
        )
        errorsItem.keyEquivalentModifierMask = [.command, .shift]
        errorsItem.target = self
        let automationItem = applicationMenu.addItem(withTitle: "Automation…", action: #selector(showAutomation(_:)), keyEquivalent: "")
        automationItem.target = self
        let securityItem = applicationMenu.addItem(withTitle: "Security…", action: #selector(showSecurity(_:)), keyEquivalent: "")
        securityItem.target = self
        let diagnosticsItem = applicationMenu.addItem(
            withTitle: "Diagnostics…",
            action: #selector(showDiagnostics(_:)),
            keyEquivalent: ""
        )
        diagnosticsItem.target = self
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
        // The processor tabs, as Show Next Tab in other Mac apps.
        let next = windowMenu.addItem(withTitle: "Show Next Processor", action: #selector(showNextProcessor(_:)), keyEquivalent: "\t")
        next.keyEquivalentModifierMask = [.control]
        next.target = self
        let previous = windowMenu.addItem(withTitle: "Show Previous Processor", action: #selector(showPreviousProcessor(_:)), keyEquivalent: "\t")
        previous.keyEquivalentModifierMask = [.control, .shift]
        previous.target = self
        windowMenu.addItem(.separator())
        windowMenu.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        windowMenu.addItem(withTitle: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        windowMenu.addItem(.separator())
        windowMenu.addItem(withTitle: "Bring All to Front", action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        windowItem.submenu = windowMenu
        mainMenu.addItem(windowItem)

        // The client's Help › Operating Manual opens the PDF its installer
        // put next to it. Orban's manual is not part of this app: the user
        // chooses it once (from the Orban installer or download) per model.
        let helpItem = NSMenuItem()
        let helpMenu = NSMenu(title: "Help")
        helpMenu.addItem(withTitle: "Operating Manual", action: #selector(showManual(_:)), keyEquivalent: "?").target = self
        helpMenu.addItem(withTitle: "Choose Operating Manual…", action: #selector(chooseManual(_:)), keyEquivalent: "").target = self
        helpItem.submenu = helpMenu
        mainMenu.addItem(helpItem)
        NSApp.helpMenu = helpMenu
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
        // The service follows the app's port and data directory.
        for key in ["OPEN_OPTIMOD_PORT", "OPTIMOD_DATA_DIR"] { environment[key] = LaunchOverride.value(key) }
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
        AppUpdater.shared.start()
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
              let path = LaunchOverride.value("OPEN_OPTIMOD_WINDOW_CONTROL")
        else { return }
        FileManager.default.createFile(atPath: path, contents: nil)
        // Common modes: the hooks also run while an alert is open.
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            guard let self, let handle = FileHandle(forReadingAtPath: path) else { return }
            defer { try? handle.close() }
            try? handle.seek(toOffset: self.windowControlOffset)
            let data = handle.readDataToEndOfFile()
            guard let text = String(data: data, encoding: .utf8), let end = text.lastIndex(of: "\n") else { return }
            let complete = text[...end]
            self.windowControlOffset += UInt64(complete.utf8.count)
            for line in complete.split(separator: "\n") {
                let control = line.trimmingCharacters(in: .whitespaces)
                if control == "diagnostics" { self.showDiagnostics(nil); continue }
                if control == "automation" { self.showAutomation(nil); continue }
                if control == "security" { self.showSecurity(nil); continue }
                if control == "manual" { self.showManual(nil); continue }
                // Test hook: `setup open|saveas NAME` acts as Open Setup / Save Setup As do.
                if control.hasPrefix("setup ") {
                    let words = control.split(separator: " ", maxSplits: 2).map(String.init)
                    if words.count == 3 { SetupPresetsPanel.perform(words[1] == "open" ? "open" : "save_as", words[2]) }
                    continue
                }
                // Test hook: `connections all|group NAME` selects that sidebar row.
                if control.hasPrefix("connections ") {
                    self.showConnections(nil)
                    self.connectionsWindowController?.perform(String(control.dropFirst("connections ".count)))
                    continue
                }
                // Test hook: `tool automation|security WORDS…` acts as the window's controls do.
                let words = control.split(separator: " ").map(String.init)
                if words.count > 2, words[0] == "tool" {
                    (words[1] == "automation" ? self.automationWindowController as ToolWindowController : self.securityWindowController)
                        .perform(Array(words.dropFirst(2)))
                    continue
                }
                // Test hook: `update check|background|install` acts as the menu items and Sparkle's daily check do.
                if control.hasPrefix("update ") {
                    switch control.dropFirst("update ".count) {
                    case "check": AppUpdater.shared.checkForUpdates(nil)
                    case "background": AppUpdater.shared.checkInBackground()
                    case "install": AppUpdater.shared.installPendingUpdate(nil)
                    default: break
                    }
                    continue
                }
                // Test hook: `press TITLE` clicks that button in the frontmost alert or window (Sparkle's too).
                if control.hasPrefix("press ") {
                    Self.press(String(control.dropFirst("press ".count)))
                    continue
                }
                self.windows.perform(control: control)
            }
            let update = try? JSONSerialization.data(withJSONObject: AppUpdater.shared.state, options: [.prettyPrinted, .sortedKeys])
            FileManager.default.createFile(atPath: path + ".update", contents: update)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                let state = try? JSONSerialization.data(withJSONObject: self.windows.windowList, options: [.prettyPrinted, .sortedKeys])
                FileManager.default.createFile(atPath: path + ".state", contents: state)
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        windowControlTimer = timer
    }

    /// The enabled button titled `title` in the modal window, else the key window, else any visible window.
    private static func press(_ title: String) {
        let ordered = [NSApp.modalWindow, NSApp.keyWindow].compactMap { $0 } + NSApp.windows.filter(\.isVisible)
        func find(_ view: NSView) -> NSButton? {
            if let button = view as? NSButton, button.title == title, button.isEnabled { return button }
            for sub in view.subviews { if let hit = find(sub) { return hit } }
            return nil
        }
        // The click runs after this callback returns: a click that opens an alert must not hold the hook timer.
        for window in ordered {
            if let view = window.contentView, let button = find(view) {
                RunLoop.main.perform(inModes: [.common]) { button.performClick(nil) }
                return
            }
        }
    }

    /// The Tools windows, one per processor (keyed by its id).
    private var diagnosticsWindows: [String: DiagnosticsWindowController] = [:]
    private var automationWindows: [String: AutomationWindowController] = [:]
    private var securityWindows: [String: SecurityWindowController] = [:]
    private var selectedKey: String { DeviceSessionStore.shared.selectedID ?? "" }
    private var selectedID: String? { DeviceSessionStore.shared.selectedID }

    private var automationWindowController: AutomationWindowController {
        let controller = automationWindows[selectedKey] ?? AutomationWindowController(deviceID: selectedID)
        automationWindows[selectedKey] = controller
        return controller
    }

    private var securityWindowController: SecurityWindowController {
        let controller = securityWindows[selectedKey] ?? SecurityWindowController(deviceID: selectedID)
        securityWindows[selectedKey] = controller
        return controller
    }

    @objc private func showAutomation(_ sender: Any?) { automationWindowController.show() }
    @objc private func showSecurity(_ sender: Any?) { securityWindowController.show() }

    @objc private func showDiagnostics(_ sender: Any?) {
        let controller = diagnosticsWindows[selectedKey] ?? DiagnosticsWindowController(deviceID: selectedID)
        diagnosticsWindows[selectedKey] = controller
        controller.show()
    }

    @objc private func showNextProcessor(_ sender: Any?) { stepProcessor(1) }
    @objc private func showPreviousProcessor(_ sender: Any?) { stepProcessor(-1) }

    private func stepProcessor(_ step: Int) {
        let store = DeviceSessionStore.shared
        guard !store.tabs.isEmpty else { return }
        let index = store.tabs.firstIndex { $0.id == store.selectedID } ?? 0
        store.select(store.tabs[(index + step + store.tabs.count) % store.tabs.count].id)
    }

    /// A tab's close button on a connected processor: only that processor's
    /// session ends; its tab closes once the service reports it disconnected.
    private func disconnect(_ id: String) {
        var request = URLRequest(url: BackendHealth.api("disconnect", device: id))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
        request.httpBody = Data("{}".utf8)
        URLSession.shared.dataTask(with: request) { data, response, _ in
            let ok = (response as? HTTPURLResponse).map { (200..<300).contains($0.statusCode) } ?? false
            let message = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }?["error"] as? String
            DispatchQueue.main.async {
                if ok {
                    DeviceSessionStore.shared.close(id)
                    DeviceSessionStore.shared.refresh()
                } else {
                    let text = message ?? "The processor could not be disconnected."
                    ErrorLogStore.shared.record(source: "Disconnect", message: text)
                    let alert = NSAlert()
                    alert.alertStyle = .warning
                    alert.messageText = "Could not disconnect"
                    alert.informativeText = text
                    if let window = NSApp.keyWindow { alert.beginSheetModal(for: window) } else { alert.runModal() }
                }
                self.connectionsWindowController?.refreshStatusNow()
            }
        }.resume()
    }

    /// Command-, opens the System Settings of the connected processor's model.
    @objc private func showSystemSettings(_ sender: Any?) {
        guard loadedInterface else { return }
        windows.showSystemSettings()
    }

    @objc private func openSetup(_ sender: Any?) { SetupPresetsPanel.open() }
    @objc private func saveSetupAs(_ sender: Any?) { SetupPresetsPanel.saveAs() }

    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(openSetup(_:)) || menuItem.action == #selector(saveSetupAs(_:)) {
            return loadedInterface && SetupPresetsPanel.available(DeviceSessionStore.shared.device?.adapterID)
        }
        if menuItem.action == #selector(showSystemSettings(_:)) {
            return loadedInterface && DeviceSessionStore.shared.device != nil
        }
        if menuItem.action == #selector(showNextProcessor(_:)) || menuItem.action == #selector(showPreviousProcessor(_:)) {
            return DeviceSessionStore.shared.tabs.count > 1
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

    /// The connected processor's model, which picks its manual.
    private var manualModel: (key: String, label: String) {
        let model = DeviceSessionStore.shared.device?.model
        return (model?.rawValue ?? "optimod", model?.label ?? "OPTIMOD")
    }

    @objc private func showManual(_ sender: Any?) {
        let key = "operatingManual.\(manualModel.key)"
        if let path = UserDefaults.standard.string(forKey: key), FileManager.default.fileExists(atPath: path) {
            NSWorkspace.shared.open(URL(fileURLWithPath: path))
        } else {
            chooseManual(sender)
        }
    }

    @objc private func chooseManual(_ sender: Any?) {
        let (key, label) = manualModel
        let open = { (url: URL) in
            UserDefaults.standard.set(url.path, forKey: "operatingManual.\(key)")
            NSWorkspace.shared.open(url)
        }
        if let url = FilePanelTestHook.nextOpen?.first {
            FilePanelTestHook.nextOpen = nil
            return open(url)
        }
        let panel = NSOpenPanel()
        panel.title = "Choose the \(label) Operating Manual"
        panel.message = "Choose the Operating Manual PDF that comes with Orban's PC Remote installer (for the 5700i: 5700i_3.0_Operating_Manual.pdf)."
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url { open(url) }
    }

    @objc private func showConnections(_ sender: Any?) {
        if connectionsWindowController == nil {
            let controller = ConnectionsWindowController()
            controller.onConnected = { [weak self] id in
                self?.windows.deviceConnected(id)
            }
            connectionsWindowController = controller
        }
        connectionsWindowController?.show()
    }

    /// Shift-Command-P opens the native Presets window of the processor,
    /// which has every preset function of its model.
    @objc private func showPresets(_ sender: Any?) {
        windows.showPresets()
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
