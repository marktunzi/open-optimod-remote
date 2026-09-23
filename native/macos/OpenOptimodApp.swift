import AppKit
import Foundation
import WebKit

final class AppDelegate: NSObject, NSApplicationDelegate, WKNavigationDelegate, WKScriptMessageHandler {
    private var window: NSWindow!
    private var webView: WKWebView!
    private var backend: Process?
    private var startedBackend = false
    private var loadedInterface = false
    private var terminationRequested = false
    private var terminationReplySent = false
    private var settingsWindowController: SystemSettingsWindowController?
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
        configureApplicationMenu()
        configureWindow()
        showStatus(title: "Starting Open Optimod Remote", detail: "Preparing the local control service…")
        startOrReuseBackend()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminationRequested else { return .terminateLater }
        terminationRequested = true

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
        NSApp.mainMenu = mainMenu
    }

    private func configureWindow() {
        let configuration = WKWebViewConfiguration()
        // The interface is shipped inside the application bundle and receives a
        // content hash on every build.  An ephemeral store prevents WebKit from
        // retaining an older shell or stylesheet after an in-place app update;
        // connection data itself remains persisted by the backend.
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.add(self, name: "openSystemSettings")
        configuration.userContentController.add(self, name: "openConnections")
        configuration.userContentController.add(self, name: "openPresets")
        configuration.userContentController.add(self, name: "recordError")
        webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = self
        webView.underPageBackgroundColor = NSColor(
            calibratedRed: 0.18,
            green: 0.18,
            blue: 0.19,
            alpha: 1
        )

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1440, height: 900),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Open Optimod Remote"
        window.minSize = NSSize(width: 1180, height: 720)
        window.contentView = webView
        window.setFrameAutosaveName("OpenOptimodRemoteMainWindow")
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
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
        webView.loadHTMLString(html, baseURL: nil)
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
        let url = URL(string: BackendHealth.origin)!
        webView.load(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData))
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

    @objc private func showSystemSettings(_ sender: Any?) {
        if settingsWindowController == nil {
            settingsWindowController = SystemSettingsWindowController()
        }
        settingsWindowController?.show()
    }

    @objc private func showConnections(_ sender: Any?) {
        if connectionsWindowController == nil {
            let controller = ConnectionsWindowController()
            controller.onConnected = { [weak self] in
                self?.window.makeKeyAndOrderFront(nil)
            }
            connectionsWindowController = controller
        }
        connectionsWindowController?.show()
    }

    @objc private func showPresets(_ sender: Any?) {
        if presetsWindowController == nil {
            let controller = PresetsWindowController()
            controller.onChanged = { [weak self] in
                self?.webView.reload()
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

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        switch message.name {
        case "openSystemSettings": showSystemSettings(nil)
        case "openConnections": showConnections(nil)
        case "openPresets": showPresets(nil)
        case "recordError":
            guard let body = message.body as? [String: Any], let text = body["message"] as? String else { break }
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

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = navigationAction.request.url else {
            decisionHandler(.cancel)
            return
        }
        if url.scheme == "about"
            || (url.scheme == "http" && url.host == "127.0.0.1" && url.port == 5701)
        {
            decisionHandler(.allow)
            return
        }
        NSWorkspace.shared.open(url)
        decisionHandler(.cancel)
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
