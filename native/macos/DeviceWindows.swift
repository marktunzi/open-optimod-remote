import AppKit
import Foundation
import WebKit

/// The top-level windows a processor has. Each one is its own native window on
/// the same device session; opening, closing or moving one never touches the
/// session.
enum DeviceWindowKind: String, CaseIterable, Sendable {
    case meters
    case processing

    var title: String {
        switch self {
        case .meters: "Meters"
        case .processing: "Processing Settings"
        }
    }

    /// Meters and Processing Settings follow the Figma frames (1133 × 370 and
    /// 1133 × 500).
    var defaultSize: NSSize {
        switch self {
        case .meters: NSSize(width: 1133, height: 370)
        case .processing: NSSize(width: 1133, height: 500)
        }
    }

    var minimumSize: NSSize {
        switch self {
        // The meters scale with the window; below this the labels get too small to read.
        case .meters: NSSize(width: 760, height: 300)
        case .processing: NSSize(width: 900, height: 360)
        }
    }

    /// The windows the app opens when it starts for the first time.
    static let startup: [DeviceWindowKind] = [.meters, .processing]
}

/// The parts of a session's state that tell the windows which processor they
/// show. The documents themselves are read by each window's own view.
struct DeviceSessionPayload: Decodable, Equatable, Sendable {
    /// The session's key in the service: the saved connection it belongs to.
    let key: String?
    let connected: Bool
    let host: String
    let firmware: String
    let deviceID: String?
    let sessionID: String?
    let adapterID: String?
    let model: String?
    let capabilities: DeviceCapabilitiesPayload?
    /// Bumped by the service whenever a document changes (a write here, the
    /// front panel, another client): windows reload their view on a change.
    let revision: Int?
    /// Set while the session waits to reconnect after an unexpected loss.
    let reconnect: ReconnectPayload?
    let error: String?

    struct ReconnectPayload: Decodable, Equatable, Sendable {
        let attempt: Int
        let attempts: Int
        let inProgress: Bool

        enum CodingKeys: String, CodingKey {
            case attempt, attempts
            case inProgress = "in_progress"
        }
    }

    enum CodingKeys: String, CodingKey {
        case key, connected, host, firmware, model, capabilities, revision, reconnect, error
        case deviceID = "device_id"
        case sessionID = "session_id"
        case adapterID = "adapter_id"
    }
}

private struct SessionsResponse: Decodable {
    let sessions: [DeviceSessionPayload]
}

/// One processor as the windows name it: one independent session per `id`.
struct DeviceRef: Equatable, Sendable {
    /// The saved connection, or the address of a connection made without one.
    /// Never a port: a processor keeps its identity when a port changes.
    let id: String
    let name: String?
    let model: ProcessorModel?
    let host: String
    let firmware: String
    let adapterID: String?
    let capabilities: DeviceCapabilitiesPayload?

    static func identity(deviceID: String?, host: String) -> String? {
        if let deviceID, !deviceID.isEmpty { return deviceID }
        return host.isEmpty ? nil : "host:\(host)"
    }

    var modelLabel: String {
        guard let model, model != .auto else { return "Optimod" }
        return model.label
    }

    /// The window subtitle: which processor this window belongs to.
    var subtitle: String {
        [name, modelLabel, host].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
    }

    /// The native System Settings window follows the 5700i worksheet only.
    /// Every other model shows the settings of its own profile.
    var usesNative5700iSettings: Bool {
        adapterID?.hasPrefix("pc-remote-5700i-") == true
    }
}

/// A processor tab: one open processor session, connected or not.
struct DeviceTab: Equatable, Sendable {
    enum Status: Equatable, Sendable { case connected, reconnecting, lost }

    let id: String
    let title: String
    let detail: String
    let status: Status
}

extension Notification.Name {
    static let deviceSessionChanged = Notification.Name("OpenOptimodDeviceSessionChanged")
}

/// The app's one reader of the service's sessions for its native windows:
/// one request a second for every processor, however many windows are open.
/// The service owns each processor's session; this store only follows them
/// and remembers which processor tab is selected.
final class DeviceSessionStore {
    static let shared = DeviceSessionStore()

    /// Every session the service runs, by processor.
    private(set) var sessions: [String: DeviceSessionPayload] = [:]
    /// The open processor tabs, in the order they were opened.
    private(set) var tabs: [DeviceTab] = []
    /// The processor of the selected tab: the one Meters and Processing
    /// Settings show and the menus act on.
    private(set) var selectedID: String?
    private var names: [String: String] = [:]
    /// Processors that were connected in this app: their tab stays after a
    /// lost connection until it is closed.
    private var opened: [String] = []
    private var lastDevices: [String: DeviceRef] = [:]
    private var timer: Timer?
    private var inFlight = false

    private lazy var urlSession: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 2
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: configuration)
    }()

    /// The selected processor while it is connected.
    var device: DeviceRef? { device(for: selectedID) }
    /// The selected processor's session.
    var session: DeviceSessionPayload? { session(for: selectedID) }

    func session(for id: String?) -> DeviceSessionPayload? {
        id.flatMap { sessions[$0] }
    }

    /// The processor `id` while its session is connected.
    func device(for id: String?) -> DeviceRef? {
        guard let id, let state = sessions[id], state.connected else { return nil }
        return makeRef(id: id, state: state)
    }

    /// The processor `id` as last seen connected (for an offline window's title).
    func lastKnown(_ id: String?) -> DeviceRef? {
        id.flatMap { lastDevices[$0] }
    }

    func start() {
        guard timer == nil else { return }
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.refresh() }
    }

    /// Selects a processor tab; nothing is connected or disconnected.
    func select(_ id: String?) {
        guard id != selectedID, id == nil || opened.contains(id!) else { return }
        selectedID = id
        NotificationCenter.default.post(name: .deviceSessionChanged, object: self)
    }

    /// A processor that was just connected opens (or selects) its tab.
    func open(_ id: String) {
        if !opened.contains(id) { opened.append(id) }
        selectedID = id
        rebuildTabs()
        NotificationCenter.default.post(name: .deviceSessionChanged, object: self)
        refresh()
    }

    /// Closes a tab whose session is no longer connected.
    func close(_ id: String) {
        opened.removeAll { $0 == id }
        if selectedID == id { selectedID = opened.last }
        rebuildTabs()
        NotificationCenter.default.post(name: .deviceSessionChanged, object: self)
    }

    func refresh() {
        guard !inFlight else { return }
        inFlight = true
        fetch("sessions", as: SessionsResponse.self) { [weak self] response in
            guard let self else { return }
            self.inFlight = false
            guard let response else { return }
            var updated: [String: DeviceSessionPayload] = [:]
            for state in response.sessions {
                guard let key = state.key ?? DeviceRef.identity(deviceID: state.deviceID, host: state.host) else { continue }
                updated[key] = state
            }
            let unknown = updated.keys.filter { self.names[$0] == nil && !$0.hasPrefix("host:") }
            if !unknown.isEmpty {
                self.fetch("devices", as: ConnectionsResponse.self) { [weak self] devices in
                    guard let self, let devices else { return }
                    for device in devices.devices { self.names[device.id] = device.name }
                    self.publish(updated, force: true)
                }
            }
            self.publish(updated, force: false)
        }
    }

    private func makeRef(id: String, state: DeviceSessionPayload) -> DeviceRef {
        DeviceRef(
            id: id,
            name: names[id],
            model: state.model.flatMap(ProcessorModel.init(rawValue:)),
            host: state.host,
            firmware: state.firmware,
            adapterID: state.adapterID,
            capabilities: state.capabilities
        )
    }

    private func publish(_ updated: [String: DeviceSessionPayload], force: Bool) {
        let changed = updated != sessions
        sessions = updated
        for (id, state) in updated where state.connected {
            lastDevices[id] = makeRef(id: id, state: state)
            if !opened.contains(id) {
                // Connected elsewhere in the app (or before it started): it gets a tab.
                opened.append(id)
                if selectedID == nil { selectedID = id }
            }
        }
        // A tab of a processor that was disconnected on purpose closes; one
        // that lost its connection stays, showing why.
        opened.removeAll { id in
            guard let state = updated[id] else { return false }
            return !state.connected && state.reconnect == nil && (state.error ?? "").isEmpty
        }
        if let selectedID, !opened.contains(selectedID) { self.selectedID = opened.last }
        let before = tabs
        rebuildTabs()
        guard force || changed || tabs != before else { return }
        NotificationCenter.default.post(name: .deviceSessionChanged, object: self)
    }

    private func rebuildTabs() {
        tabs = opened.map { id in
            let state = sessions[id]
            let ref = lastDevices[id]
            let status: DeviceTab.Status = state?.connected == true ? .connected : state?.reconnect != nil ? .reconnecting : .lost
            return DeviceTab(id: id, title: names[id] ?? ref?.host ?? id, detail: ref?.modelLabel ?? "Optimod", status: status)
        }
    }

    private func fetch<T: Decodable>(_ path: String, as type: T.Type, completion: @escaping (T?) -> Void) {
        let request = URLRequest(url: BackendHealth.api(path), cachePolicy: .reloadIgnoringLocalCacheData)
        urlSession.dataTask(with: request) { data, response, _ in
            let ok = (response as? HTTPURLResponse)?.statusCode == 200
            let value = ok ? data.flatMap { try? JSONDecoder().decode(T.self, from: $0) } : nil
            DispatchQueue.main.async { completion(value) }
        }.resume()
    }
}

/// Allows only the bundled interface inside the app's web views; every other
/// link opens in the default browser.
enum BackendNavigation {
    static func allows(_ url: URL) -> Bool {
        url.scheme == "about"
            || (url.scheme == "http" && url.host == "127.0.0.1" && url.port == BackendHealth.port)
    }

    static func decide(_ action: WKNavigationAction, _ decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else {
            decisionHandler(.cancel)
            return
        }
        // A file the page saves (backup, preset file) is a download the
        // window asks where to put.
        if action.shouldPerformDownload || url.scheme == "blob" {
            decisionHandler(.download)
            return
        }
        if allows(url) {
            decisionHandler(.allow)
            return
        }
        NSWorkspace.shared.open(url)
        decisionHandler(.cancel)
    }
}

/// Messages from a window's page to the app, tagged with the page that sent them.
protocol DeviceWindowBridgeDelegate: AnyObject {
    func deviceWindow(_ webView: WKWebView?, didRequest name: String, body: Any)
}

/// Receives the page messages of every window. WebKit keeps its message
/// handlers alive, so this object holds the app only weakly.
final class DeviceWindowBridge: NSObject, WKScriptMessageHandler {
    static let messages = ["openSystemSettings", "openConnections", "openPresets", "openMeters", "openProcessing", "recordError", "meterSize"]
    weak var delegate: DeviceWindowBridgeDelegate?

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        delegate?.deviceWindow(message.webView, didRequest: message.name, body: message.body)
    }
}

/// A window that shows one page of the interface for one processor.
/// Test hook (OPEN_OPTIMOD_WINDOW_CONTROL): files for the next open panel and
/// the folder for the next save, so a test needs no mouse on the panels.
enum FilePanelTestHook {
    static var nextOpen: [URL]?
    static var nextSaveFolder: URL?
}

final class DeviceWebWindowController: NSWindowController, NSWindowDelegate, WKNavigationDelegate, WKUIDelegate, WKDownloadDelegate {
    let kind: DeviceWindowKind
    /// The processor this window shows: the selected tab's; nil until one is open.
    private(set) var deviceID: String?
    let webView: WKWebView
    /// The processor tabs above the page.
    let tabBar = DeviceTabBar(frame: NSRect(x: 0, y: 0, width: 800, height: DeviceTabBar.height))
    var onClose: ((DeviceWebWindowController) -> Void)?
    private var loadedPage = false

    init(kind: DeviceWindowKind, deviceID: String?, configuration: WKWebViewConfiguration) {
        self.kind = kind
        self.deviceID = deviceID
        webView = WKWebView(frame: .zero, configuration: configuration)
        webView.underPageBackgroundColor = NSColor(calibratedRed: 0.18, green: 0.18, blue: 0.19, alpha: 1)
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: kind.defaultSize),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = kind.title
        window.minSize = kind.minimumSize
        window.isReleasedWhenClosed = false
        window.tabbingMode = .disallowed
        // The Figma design is dark from edge to edge; the title bar takes the
        // colour of the processor tabs below it.
        window.appearance = NSAppearance(named: .darkAqua)
        window.titlebarAppearsTransparent = true
        window.backgroundColor = DeviceTabBar.background
        window.contentView = webView
        window.identifier = NSUserInterfaceItemIdentifier("OpenOptimod.\(kind.rawValue)")
        let tabs = NSTitlebarAccessoryViewController()
        tabs.view = tabBar
        tabs.layoutAttribute = .bottom
        window.addTitlebarAccessoryViewController(tabs)
        super.init(window: window)
        window.delegate = self
        webView.navigationDelegate = self
        webView.uiDelegate = self
        restoreFrame()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// The window keeps its place when another processor tab is chosen; a
    /// first window takes the last position of its kind.
    private func restoreFrame() {
        guard let window else { return }
        let name = Self.autosaveName(kind: kind)
        if !window.setFrameUsingName(name) {
            if !window.setFrameUsingName(Self.lastFrameName(kind)) { placeFirstTime(window) }
        }
        // Naming the frame applies a saved one, so the size check comes after.
        window.setFrameAutosaveName(name)
        enforceOpeningSize()
    }

    /// The reference size, and for Meters the size its meters need.
    private var meterContent: NSSize?
    private func enforceOpeningSize() {
        guard let window else { return }
        grow(window, toContent: kind.defaultSize)
        if let meterContent { grow(window, toContent: meterContent) }
    }

    /// A window never opens smaller than this content size (capped to its
    /// screen); a smaller saved frame grows from its top-left corner.
    func grow(_ window: NSWindow, toContent wanted: NSSize) {
        guard let visible = (window.screen ?? NSScreen.main ?? NSScreen.screens.first)?.visibleFrame else { return }
        let content = window.contentRect(forFrameRect: window.frame)
        let chrome = window.frame.height - content.height
        let target = NSSize(width: min(max(content.width, wanted.width), visible.width),
                            height: min(max(content.height, wanted.height), visible.height - chrome))
        guard target.width > content.width + 0.5 || target.height > content.height + 0.5 else { return }
        var frame = window.frameRect(forContentRect: NSRect(origin: content.origin, size: target))
        frame.origin.y = window.frame.maxY - frame.height
        frame.origin.x = min(max(frame.origin.x, visible.minX), max(visible.minX, visible.maxX - frame.width))
        frame.origin.y = min(max(frame.origin.y, visible.minY), max(visible.minY, visible.maxY - frame.height))
        window.setFrame(frame, display: true)
    }

    /// The Meters page reports the size its meters need at their real size;
    /// the window opens that large once per page load. A smaller window later
    /// shows scroll bars, the meters are never scaled.
    private var fittedMeters = false
    func fitMeters(width: CGFloat, height: CGFloat) {
        guard kind == .meters, !fittedMeters, width > 0, height > 0 else { return }
        fittedMeters = true
        meterContent = NSSize(width: width, height: height)
        enforceOpeningSize()
    }

    /// The first time, Meters sits at the top of the screen with Processing
    /// Settings directly below it, as one instrument; other windows centre.
    private func placeFirstTime(_ window: NSWindow) {
        guard let visible = (NSScreen.main ?? NSScreen.screens.first)?.visibleFrame,
              kind == .meters || kind == .processing
        else {
            window.center()
            return
        }
        let meters = window.frameRect(forContentRect: NSRect(origin: .zero, size: DeviceWindowKind.meters.defaultSize))
        var frame = window.frame
        frame.origin.x = visible.midX - frame.width / 2
        let top = visible.maxY - 24 - (kind == .processing ? meters.height + 12 : 0)
        frame.origin.y = max(visible.minY, top - frame.height)
        window.setFrame(frame, display: false)
    }

    static func autosaveName(kind: DeviceWindowKind) -> String {
        "OpenOptimod.\(kind.rawValue).tabs"
    }

    private static func lastFrameName(_ kind: DeviceWindowKind) -> String {
        "OpenOptimod.\(kind.rawValue).last"
    }

    func loadPage() {
        loadedPage = true
        fittedMeters = false
        var components = URLComponents(string: BackendHealth.origin)!
        components.path = "/"
        components.queryItems = [URLQueryItem(name: "window", value: kind.rawValue)]
            + (deviceID.map { [URLQueryItem(name: "device", value: $0)] } ?? [])
        webView.load(URLRequest(url: components.url!, cachePolicy: .reloadIgnoringLocalCacheData))
    }

    var hasPage: Bool { loadedPage }

    func showStatus(html: String) {
        webView.loadHTMLString(html, baseURL: nil)
    }

    /// Shows another processor (a tab was chosen). Its page reads only that
    /// processor's session; the other sessions are not touched.
    func bind(deviceID: String?) {
        guard deviceID != self.deviceID else { return }
        self.deviceID = deviceID
        if loadedPage { loadPage() }
    }

    func update(store: DeviceSessionStore) {
        guard let window else { return }
        if let device = store.device(for: deviceID) {
            window.subtitle = device.subtitle
        } else if let known = store.lastKnown(deviceID) {
            window.subtitle = "\(known.subtitle) · \(store.session(for: deviceID)?.reconnect != nil ? "Reconnecting" : "Offline")"
        } else {
            window.subtitle = "Not connected"
        }
        window.title = kind.title
        tabBar.update(tabs: store.tabs, selected: store.selectedID)
    }

    func show() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
    }

    func windowDidMove(_ notification: Notification) { rememberFrame() }
    func windowDidEndLiveResize(_ notification: Notification) { rememberFrame() }

    private func rememberFrame() {
        window?.saveFrame(usingName: Self.lastFrameName(kind))
    }

    func windowWillClose(_ notification: Notification) {
        rememberFrame()
        // The page and its meter reader end with the window; the session stays.
        webView.stopLoading()
        webView.loadHTMLString("", baseURL: nil)
        onClose?(self)
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        BackendNavigation.decide(navigationAction, decisionHandler)
    }

    func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
        download.delegate = self
    }

    func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
        download.delegate = self
    }

    /// The page's file inputs (Apply preset file, Restore) open a native panel.
    func webView(
        _ webView: WKWebView,
        runOpenPanelWith parameters: WKOpenPanelParameters,
        initiatedByFrame frame: WKFrameInfo,
        completionHandler: @escaping ([URL]?) -> Void
    ) {
        if let files = FilePanelTestHook.nextOpen {
            FilePanelTestHook.nextOpen = nil
            completionHandler(files)
            return
        }
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = parameters.allowsMultipleSelection
        panel.canChooseDirectories = false
        guard let window else { return completionHandler(nil) }
        panel.beginSheetModal(for: window) { response in
            completionHandler(response == .OK ? panel.urls : nil)
        }
    }

    func download(
        _ download: WKDownload,
        decideDestinationUsing response: URLResponse,
        suggestedFilename: String,
        completionHandler: @escaping (URL?) -> Void
    ) {
        if let folder = FilePanelTestHook.nextSaveFolder {
            FilePanelTestHook.nextSaveFolder = nil
            completionHandler(folder.appendingPathComponent(suggestedFilename))
            return
        }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = suggestedFilename
        guard let window else { return completionHandler(nil) }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return completionHandler(nil) }
            // WebKit does not replace a file; the panel already asked.
            try? FileManager.default.removeItem(at: url)
            completionHandler(url)
        }
    }
}

/// Opens and keeps the app's windows. Meters and Processing Settings show
/// the processor of the selected tab; Presets and System Settings are one
/// window per processor. A window never shows another processor's data.
final class DeviceWindowManager: DeviceWindowBridgeDelegate {
    private static let openKindsDefault = "OpenOptimodOpenWindows"
    private static let presetsKind = "presets"

    private var windows: [DeviceWindowKind: DeviceWebWindowController] = [:]
    private var nativeSettings: [String: SystemSettingsWindowController] = [:]
    /// Presets is a native window per processor, like System Settings.
    private var presets: [String: PresetsWindowController] = [:]
    var onPresetsChanged: (() -> Void)?
    /// A tab's close button on a connected processor: the app disconnects it.
    var onDisconnect: ((String) -> Void)?
    private let bridge = DeviceWindowBridge()
    private let dataStore = WKWebsiteDataStore.nonPersistent()
    private var interfaceReady = false
    private var terminating = false
    weak var appDelegate: DeviceWindowBridgeDelegate?

    init() {
        bridge.delegate = self
        NotificationCenter.default.addObserver(
            forName: .deviceSessionChanged, object: nil, queue: .main
        ) { [weak self] _ in self?.sessionChanged() }
    }

    private var store: DeviceSessionStore { .shared }

    /// The processor new windows belong to: the selected tab's.
    private var currentDeviceID: String? { store.selectedID }

    private func makeConfiguration() -> WKWebViewConfiguration {
        let configuration = WKWebViewConfiguration()
        // The interface is in the bundle with a content hash per build; a
        // non-persistent store never keeps an older shell after an update.
        configuration.websiteDataStore = dataStore
        for name in DeviceWindowBridge.messages {
            configuration.userContentController.add(bridge, name: name)
        }
        return configuration
    }

    /// Shows a window of this kind for the selected processor, opening it if needed.
    @discardableResult
    func show(_ kind: DeviceWindowKind) -> DeviceWebWindowController {
        let controller = windows[kind] ?? makeWindow(kind)
        controller.bind(deviceID: currentDeviceID)
        if interfaceReady && !controller.hasPage { controller.loadPage() }
        controller.update(store: store)
        controller.show()
        rememberOpenKinds()
        return controller
    }

    private func makeWindow(_ kind: DeviceWindowKind) -> DeviceWebWindowController {
        let controller = DeviceWebWindowController(kind: kind, deviceID: currentDeviceID, configuration: makeConfiguration())
        controller.onClose = { [weak self] closed in self?.closed(closed) }
        controller.tabBar.onSelect = { [weak self] id in self?.store.select(id) }
        controller.tabBar.onClose = { [weak self, weak controller] id in self?.closeTab(id, from: controller?.window) }
        controller.tabBar.onAdd = { [weak self] in self?.appDelegate?.deviceWindow(nil, didRequest: "openConnections", body: [:]) }
        windows[kind] = controller
        return controller
    }

    private func closed(_ controller: DeviceWebWindowController) {
        windows = windows.filter { $0.value !== controller }
        if !terminating { rememberOpenKinds() }
    }

    func controller(for webView: WKWebView?) -> DeviceWebWindowController? {
        windows.values.first { $0.webView === webView }
    }

    /// A tab's close button: a processor that is still connected (or
    /// reconnecting) is disconnected after a confirmation; a lost one's tab closes.
    func closeTab(_ id: String, from window: NSWindow?) {
        let tab = store.tabs.first { $0.id == id }
        guard let tab, tab.status != .lost else {
            store.close(id)
            return
        }
        let alert = NSAlert()
        alert.messageText = "Disconnect \(tab.title)?"
        alert.informativeText = "The other processors stay connected."
        alert.addButton(withTitle: "Disconnect")
        alert.addButton(withTitle: "Cancel")
        let disconnect = { [weak self] (response: NSApplication.ModalResponse) in
            guard response == .alertFirstButtonReturn else { return }
            self?.onDisconnect?(id)
        }
        if let window { alert.beginSheetModal(for: window, completionHandler: disconnect) } else { disconnect(alert.runModal()) }
    }

    /// System Settings is always a native window: the 5700i worksheet, or
    /// the System fields of the selected processor's own profile.
    func showSystemSettings() {
        guard let device = store.device else { return }
        let controller = nativeSettings[device.id] ?? SystemSettingsWindowController(device: device)
        nativeSettings[device.id] = controller
        controller.show()
    }

    /// The Presets window of the selected processor.
    func showPresets() {
        let key = currentDeviceID ?? ""
        let controller = presets[key] ?? {
            let controller = PresetsWindowController(deviceID: currentDeviceID)
            controller.onChanged = { [weak self] in self?.onPresetsChanged?() }
            controller.onClose = { [weak self] closed in
                guard let self else { return }
                self.presets = self.presets.filter { $0.value !== closed }
                if !self.terminating { self.rememberOpenKinds() }
            }
            return controller
        }()
        presets[key] = controller
        controller.show()
        rememberOpenKinds()
    }

    /// The startup window shows the service's progress until the interface loads.
    func showStartup(html: String) {
        show(.meters).showStatus(html: html)
    }

    func interfaceDidLoad() {
        interfaceReady = true
        store.start()
        let kinds = UserDefaults.standard.stringArray(forKey: Self.openKindsDefault)?
            .compactMap(DeviceWindowKind.init(rawValue:)) ?? DeviceWindowKind.startup
        for kind in kinds.isEmpty ? [.meters] : kinds { show(kind) }
        for controller in windows.values where !controller.hasPage { controller.loadPage() }
        if UserDefaults.standard.stringArray(forKey: Self.openKindsDefault)?.contains(Self.presetsKind) == true { showPresets() }
    }

    /// After a connection, the processor gets its tab and the main windows come forward.
    func deviceConnected(_ id: String) {
        store.open(id)
        for kind in [DeviceWindowKind.processing, .meters] { show(kind) }
    }

    func prepareForTermination() {
        terminating = true
        rememberOpenKinds()
    }

    var hasVisibleWindows: Bool {
        windows.values.contains { $0.window?.isVisible == true }
    }

    private func rememberOpenKinds() {
        // The startup window shows progress only; it is not a choice to keep.
        guard interfaceReady else { return }
        let kinds = DeviceWindowKind.allCases.filter { windows[$0] != nil }
        UserDefaults.standard.set(kinds.map(\.rawValue) + (presets.isEmpty ? [] : [Self.presetsKind]), forKey: Self.openKindsDefault)
    }

    /// Meters and Processing Settings follow the selected tab; a Presets
    /// window opened before any processor belongs to the first one.
    private func sessionChanged() {
        if let selected = store.selectedID, let unbound = presets[""] {
            presets[""] = nil
            unbound.adopt(deviceID: selected)
            presets[selected] = unbound
        }
        for controller in windows.values {
            controller.bind(deviceID: store.selectedID)
            controller.update(store: store)
        }
    }

    /// Test hook (OPEN_OPTIMOD_WINDOW_CONTROL=<file>): the same AppKit window
    /// actions as the title bar and Window menu, appended to the file as lines
    /// `show|close|minimize|restore KIND`, `frame KIND X Y W H` or `settings`;
    /// `tab KIND N` presses the Nth page tab (from 0) and `path KIND FM|HD` the
    /// processing path, as a click on them does; `settings-tab N` selects a
    /// System Settings tab.
    /// Every line also writes the window list to <file>.state.
    func perform(control line: String) {
        let parts = line.split(separator: " ").map(String.init)
        guard let verb = parts.first else { return }
        let kind = parts.count > 1 ? DeviceWindowKind(rawValue: parts[1]) : nil
        let controller = kind.flatMap { windows[$0] }
        switch verb {
        // Test hook: `tab-select N|ID` chooses a processor tab as a click does,
        // `tab-close N|ID` presses its close button (`confirm` answers the sheet).
        case "tab-select" where parts.count > 1:
            store.select(tabID(parts[1]))
        case "tab-close" where parts.count > 1:
            if let id = tabID(parts[1]) {
                if parts.count > 2 && parts[2] == "confirm", store.tabs.first(where: { $0.id == id })?.status != .lost {
                    onDisconnect?(id)
                } else {
                    closeTab(id, from: windows[.meters]?.window)
                }
            }
        case "show" where parts.count > 1 && parts[1] == Self.presetsKind: showPresets()
        case "close" where parts.count > 1 && parts[1] == Self.presetsKind: presets.values.forEach { $0.close() }
        case Self.presetsKind: presets.values.forEach { $0.perform(Array(parts.dropFirst())) }
        case "show": if let kind { show(kind) }
        case "settings": showSystemSettings()
        case "close" where parts.count > 1 && parts[1] == "settings":
            nativeSettings.values.forEach { $0.close() }
        case "close": controller?.window?.performClose(nil)
        case "minimize": controller?.window?.miniaturize(nil)
        case "restore": controller?.window?.deminiaturize(nil)
        case "tab" where parts.count > 2:
            controller?.webView.evaluateJavaScript(
                "document.querySelectorAll('nav.tabs > button')[\(Int(parts[2]) ?? 0)]?.click()")
        case "path" where parts.count > 2 && ["FM", "HD"].contains(parts[2]):
            controller?.webView.evaluateJavaScript(
                "[...document.querySelectorAll('.processing-path-selector button')].find(b => b.textContent === '\(parts[2])')?.click()")
        case "click" where parts.count > 2:
            // Test hook: the first button whose label starts with TEXT.
            let text = parts.dropFirst(2).joined(separator: " ").replacingOccurrences(of: "'", with: "\\'")
            controller?.webView.evaluateJavaScript(
                "[...document.querySelectorAll('button')].find(b => b.textContent.trim().startsWith('\(text)') && !b.disabled)?.click()")
        case "type" where parts.count > 2:
            // Test hook: TEXT into the input named FIELD (`+` for a space), as typing does for React.
            let text = parts.dropFirst(3).joined(separator: " ").replacingOccurrences(of: "'", with: "\\'")
            controller?.webView.evaluateJavaScript(
                "(() => { const i = document.querySelector('[name=\"\(parts[2].replacingOccurrences(of: "+", with: " "))\"]'); if (!i) return; const proto = i instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(proto, 'value').set.call(i, '\(text)'); i.dispatchEvent(new Event(i instanceof HTMLSelectElement ? 'change' : 'input', {bubbles: true})); })()")
        case "enter" where parts.count > 2:
            // Test hook: Return in the input named FIELD, as pressing it does.
            controller?.webView.evaluateJavaScript(
                "document.querySelector('[name=\"\(parts[2].replacingOccurrences(of: "+", with: " "))\"]')?.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}))")
        case "files" where parts.count > 1:
            // Test hook: `files PATH|PATH…` answers the next open panel.
            FilePanelTestHook.nextOpen = parts.dropFirst().joined(separator: " ")
                .split(separator: "|").map { URL(fileURLWithPath: String($0)) }
        case "saveto" where parts.count > 1:
            FilePanelTestHook.nextSaveFolder = URL(fileURLWithPath: parts.dropFirst().joined(separator: " "))
        case "settings-tab" where parts.count > 1:
            nativeSettings.values.forEach { $0.selectTab(at: Int(parts[1]) ?? 0) }
        case "frame":
            let numbers = parts.dropFirst(2).compactMap(Double.init)
            if numbers.count == 4 {
                controller?.window?.setFrame(NSRect(x: numbers[0], y: numbers[1], width: numbers[2], height: numbers[3]), display: true)
            }
        default: break
        }
    }

    private func tabID(_ word: String) -> String? {
        if let index = Int(word) { return store.tabs.indices.contains(index) ? store.tabs[index].id : nil }
        return store.tabs.contains { $0.id == word } ? word : nil
    }

    /// The open windows and the processor tabs, for the test hook.
    var windowList: [[String: Any]] {
        [["kind": "tabs", "selected": store.selectedID ?? "",
          "tabs": store.tabs.map { ["id": $0.id, "title": $0.title, "detail": $0.detail, "status": "\($0.status)"] }]]
        + windows.map { kind, controller in
            let frame = controller.window?.frame ?? .zero
            return [
                "kind": kind.rawValue,
                "device": controller.deviceID ?? "",
                "visible": controller.window?.isVisible ?? false,
                "minimized": controller.window?.isMiniaturized ?? false,
                "subtitle": controller.window?.subtitle ?? "",
                "window_number": controller.window?.windowNumber ?? 0,
                "frame": [frame.origin.x, frame.origin.y, frame.width, frame.height],
            ]
        } + presets.values.map(\.testState) + nativeSettings.map { id, controller in
            ["kind": "native-system-settings", "device": id, "visible": controller.window?.isVisible ?? false,
             "window_number": controller.window?.windowNumber ?? 0]
        }
    }

    func deviceWindow(_ webView: WKWebView?, didRequest name: String, body: Any) {
        switch name {
        case "openSystemSettings": showSystemSettings()
        case "openPresets": showPresets()
        case "openMeters": show(.meters)
        case "openProcessing": show(.processing)
        case "meterSize":
            if let size = body as? [String: Any], let width = (size["width"] as? NSNumber)?.doubleValue,
               let height = (size["height"] as? NSNumber)?.doubleValue {
                controller(for: webView)?.fitMeters(width: width, height: height)
            }
        default: appDelegate?.deviceWindow(webView, didRequest: name, body: body)
        }
    }
}
