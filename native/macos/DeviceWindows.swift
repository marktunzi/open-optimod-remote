import AppKit
import Foundation
import WebKit

/// The top-level windows a processor has. Each one is its own native window on
/// the same device session; opening, closing or moving one never touches the
/// session.
enum DeviceWindowKind: String, CaseIterable, Sendable {
    case meters
    case processing
    case presets

    var title: String {
        switch self {
        case .meters: "Meters"
        case .processing: "Processing Settings"
        case .presets: "Presets"
        }
    }

    /// Meters and Processing Settings follow the Figma frames (1133 × 370 and
    /// 1133 × 500).
    var defaultSize: NSSize {
        switch self {
        case .meters: NSSize(width: 1133, height: 370)
        case .processing: NSSize(width: 1133, height: 500)
        case .presets: NSSize(width: 1000, height: 640)
        }
    }

    /// The content size a window opens at on a screen this wide. The meters
    /// scale evenly with the window, so Meters opens at the Figma proportions
    /// across most of the screen (at least the 1133 × 370 reference); the
    /// other windows open at their reference size.
    func openingSize(screenWidth: CGFloat) -> NSSize {
        guard self == .meters else { return defaultSize }
        let width = min(1700, max(defaultSize.width, (screenWidth * 0.85).rounded()))
        return NSSize(width: width, height: (width * defaultSize.height / defaultSize.width).rounded())
    }

    var minimumSize: NSSize {
        switch self {
        // The meters scale with the window; below this the labels get too small to read.
        case .meters: NSSize(width: 760, height: 300)
        case .processing: NSSize(width: 900, height: 360)
        case .presets: NSSize(width: 760, height: 460)
        }
    }

    /// The windows the app opens when it starts for the first time.
    static let startup: [DeviceWindowKind] = [.meters, .processing]
}

/// What a processor offers, as reported by its adapter. A window that needs a
/// capability the processor lacks shows that instead of guessing.
struct DeviceCapabilitiesPayload: Decodable, Equatable, Sendable {
    let parameterReads: Bool
    let parameterWrites: Bool
    let liveMeters: Bool
    let presetCatalog: Bool

    enum CodingKeys: String, CodingKey {
        case parameterReads = "parameter_reads"
        case parameterWrites = "parameter_writes"
        case liveMeters = "live_meters"
        case presetCatalog = "preset_catalog"
    }
}

/// The parts of the service state that tell the windows which processor they
/// show. The documents themselves are read by each window's own view.
struct DeviceSessionPayload: Decodable, Equatable, Sendable {
    let connected: Bool
    let host: String
    let firmware: String
    let deviceID: String?
    let sessionID: String?
    let adapterID: String?
    let model: String?
    let capabilities: DeviceCapabilitiesPayload?

    enum CodingKeys: String, CodingKey {
        case connected, host, firmware, model, capabilities
        case deviceID = "device_id"
        case sessionID = "session_id"
        case adapterID = "adapter_id"
    }
}

/// One processor as the windows name it. Phase 6 keeps one session per
/// `id`; until then the app has at most one connected processor.
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

extension Notification.Name {
    static let deviceSessionChanged = Notification.Name("OpenOptimodDeviceSessionChanged")
}

/// The app's one reader of the service state for its native windows. The
/// service owns the only processor session; this store only follows it.
final class DeviceSessionStore {
    static let shared = DeviceSessionStore()

    private(set) var session: DeviceSessionPayload?
    /// The connected processor, nil while none is connected.
    private(set) var device: DeviceRef?
    private var names: [String: String] = [:]
    private var timer: Timer?
    private var inFlight = false

    private lazy var urlSession: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 2
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: configuration)
    }()

    func start() {
        guard timer == nil else { return }
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.refresh() }
    }

    func refresh() {
        guard !inFlight else { return }
        inFlight = true
        fetch("state", as: DeviceSessionPayload.self) { [weak self] state in
            guard let self else { return }
            self.inFlight = false
            guard let state else { return }
            let identity = DeviceRef.identity(deviceID: state.deviceID, host: state.host)
            if let deviceID = state.deviceID, self.names[deviceID] == nil {
                self.fetch("devices", as: ConnectionsResponse.self) { [weak self] response in
                    guard let self, let response else { return }
                    for device in response.devices { self.names[device.id] = device.name }
                    self.publish(state, identity: identity, force: true)
                }
            }
            self.publish(state, identity: identity, force: false)
        }
    }

    private func publish(_ state: DeviceSessionPayload, identity: String?, force: Bool) {
        let device: DeviceRef? = state.connected ? identity.map {
            DeviceRef(
                id: $0,
                name: state.deviceID.flatMap { names[$0] },
                model: state.model.flatMap(ProcessorModel.init(rawValue:)),
                host: state.host,
                firmware: state.firmware,
                adapterID: state.adapterID,
                capabilities: state.capabilities
            )
        } : nil
        guard force || state != session || device != self.device else { return }
        session = state
        self.device = device
        NotificationCenter.default.post(name: .deviceSessionChanged, object: self)
    }

    private func fetch<T: Decodable>(_ path: String, as type: T.Type, completion: @escaping (T?) -> Void) {
        let request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!, cachePolicy: .reloadIgnoringLocalCacheData)
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
    static let messages = ["openSystemSettings", "openConnections", "openPresets", "openMeters", "openProcessing", "recordError"]
    weak var delegate: DeviceWindowBridgeDelegate?

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        delegate?.deviceWindow(message.webView, didRequest: message.name, body: message.body)
    }
}

/// A window that shows one page of the interface for one processor.
final class DeviceWebWindowController: NSWindowController, NSWindowDelegate, WKNavigationDelegate {
    let kind: DeviceWindowKind
    /// The processor this window belongs to; nil until one is connected.
    private(set) var deviceID: String?
    let webView: WKWebView
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
        // The Figma design is dark from edge to edge; the title bar follows it.
        window.appearance = NSAppearance(named: .darkAqua)
        window.titlebarAppearsTransparent = true
        window.backgroundColor = NSColor(srgbRed: 0x4c / 255, green: 0x4c / 255, blue: 0x4c / 255, alpha: 1)
        window.contentView = webView
        window.identifier = NSUserInterfaceItemIdentifier("OpenOptimod.\(kind.rawValue)")
        super.init(window: window)
        window.delegate = self
        webView.navigationDelegate = self
        restoreFrame()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// Each processor keeps its own window positions. A window for a processor
    /// that has none yet takes the last position of its kind.
    private func restoreFrame() {
        guard let window else { return }
        let name = Self.autosaveName(kind: kind, deviceID: deviceID)
        if !window.setFrameUsingName(name) {
            if !window.setFrameUsingName(Self.lastFrameName(kind)) { placeFirstTime(window) }
        }
        growToOpeningSize(window)
        window.setFrameAutosaveName(name)
    }

    /// A window never opens smaller than its opening size; a smaller saved
    /// frame grows from its top-left corner and stays on its screen.
    private func growToOpeningSize(_ window: NSWindow) {
        guard let visible = (window.screen ?? NSScreen.main ?? NSScreen.screens.first)?.visibleFrame else { return }
        let content = window.contentRect(forFrameRect: window.frame)
        let wanted = kind.openingSize(screenWidth: visible.width)
        guard content.width < wanted.width - 0.5 || content.height < wanted.height - 0.5 else { return }
        var frame = window.frameRect(forContentRect: NSRect(origin: content.origin,
            size: NSSize(width: max(content.width, wanted.width), height: max(content.height, wanted.height))))
        frame.origin.y = window.frame.maxY - frame.height
        frame.origin.x = min(max(frame.origin.x, visible.minX), max(visible.minX, visible.maxX - frame.width))
        frame.origin.y = min(max(frame.origin.y, visible.minY), max(visible.minY, visible.maxY - frame.height))
        window.setFrame(frame, display: false)
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
        let meters = window.frameRect(forContentRect: NSRect(origin: .zero,
            size: DeviceWindowKind.meters.openingSize(screenWidth: visible.width)))
        var frame = window.frameRect(forContentRect: NSRect(origin: .zero, size: kind.openingSize(screenWidth: visible.width)))
        frame.origin.x = visible.midX - frame.width / 2
        let top = visible.maxY - 24 - (kind == .processing ? meters.height + 12 : 0)
        frame.origin.y = max(visible.minY, top - frame.height)
        window.setFrame(frame, display: false)
    }

    static func autosaveName(kind: DeviceWindowKind, deviceID: String?) -> String {
        "OpenOptimod.\(kind.rawValue).\(deviceID ?? "none")"
    }

    private static func lastFrameName(_ kind: DeviceWindowKind) -> String {
        "OpenOptimod.\(kind.rawValue).last"
    }

    func loadPage() {
        loadedPage = true
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

    /// Binds a window that was opened before any processor was connected.
    func adopt(deviceID: String) {
        guard self.deviceID == nil, let window else { return }
        self.deviceID = deviceID
        window.setFrameAutosaveName(Self.autosaveName(kind: kind, deviceID: deviceID))
        if loadedPage { loadPage() }
    }

    func update(device: DeviceRef?, connectedDevice: DeviceRef?) {
        guard let window else { return }
        if let device {
            window.subtitle = connectedDevice?.id == device.id ? device.subtitle : "\(device.subtitle) · Offline"
        } else if let connectedDevice, deviceID == nil {
            window.subtitle = connectedDevice.subtitle
        } else {
            window.subtitle = "Not connected"
        }
        window.title = kind.title
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
}

/// Opens and keeps the windows of each processor. Windows are keyed by kind
/// and processor, so a processor's windows never show another one's data.
final class DeviceWindowManager: DeviceWindowBridgeDelegate {
    private struct Key: Hashable {
        let kind: DeviceWindowKind
        let deviceID: String?
    }

    private static let openKindsDefault = "OpenOptimodOpenWindows"

    private var windows: [Key: DeviceWebWindowController] = [:]
    private var nativeSettings: [String: SystemSettingsWindowController] = [:]
    private let bridge = DeviceWindowBridge()
    private let dataStore = WKWebsiteDataStore.nonPersistent()
    private var interfaceReady = false
    private var terminating = false
    private var lastDevice: DeviceRef?
    weak var appDelegate: DeviceWindowBridgeDelegate?

    init() {
        bridge.delegate = self
        NotificationCenter.default.addObserver(
            forName: .deviceSessionChanged, object: nil, queue: .main
        ) { [weak self] _ in self?.sessionChanged() }
    }

    private var store: DeviceSessionStore { .shared }

    /// The processor new windows belong to: the connected one, else the last one.
    private var currentDeviceID: String? { store.device?.id ?? lastDevice?.id }

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

    /// Shows a window of this kind for the current processor, opening it if needed.
    @discardableResult
    func show(_ kind: DeviceWindowKind) -> DeviceWebWindowController {
        let key = Key(kind: kind, deviceID: currentDeviceID)
        let controller = windows[key] ?? makeWindow(key)
        if interfaceReady && !controller.hasPage { controller.loadPage() }
        controller.update(device: device(for: key.deviceID), connectedDevice: store.device)
        controller.show()
        rememberOpenKinds()
        return controller
    }

    private func makeWindow(_ key: Key) -> DeviceWebWindowController {
        let controller = DeviceWebWindowController(kind: key.kind, deviceID: key.deviceID, configuration: makeConfiguration())
        controller.onClose = { [weak self] closed in self?.closed(closed) }
        windows[key] = controller
        return controller
    }

    private func closed(_ controller: DeviceWebWindowController) {
        windows = windows.filter { $0.value !== controller }
        if !terminating { rememberOpenKinds() }
    }

    func controller(for webView: WKWebView?) -> DeviceWebWindowController? {
        windows.values.first { $0.webView === webView }
    }

    /// System Settings is always a native window: the 5700i worksheet, or
    /// the System fields of the connected model's own profile.
    func showSystemSettings() {
        guard let device = store.device else { return }
        let controller = nativeSettings[device.id] ?? SystemSettingsWindowController()
        nativeSettings[device.id] = controller
        controller.show()
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
    }

    /// After a connection, the processor's main windows come forward.
    func deviceConnected() {
        store.refresh()
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
        let kinds = DeviceWindowKind.allCases.filter { kind in windows.keys.contains { $0.kind == kind } }
        UserDefaults.standard.set(kinds.map(\.rawValue), forKey: Self.openKindsDefault)
    }

    private func device(for id: String?) -> DeviceRef? {
        guard let id else { return nil }
        if store.device?.id == id { return store.device }
        return lastDevice?.id == id ? lastDevice : nil
    }

    /// One processor at a time until Phase 6: windows of a processor that is
    /// no longer the connected one close, and the same kinds open for the new
    /// one. A disconnect alone keeps every window, so it comes back on reconnect.
    private func sessionChanged() {
        let connected = store.device
        if let connected {
            for (key, controller) in windows where key.deviceID == nil {
                windows[key] = nil
                controller.adopt(deviceID: connected.id)
                windows[Key(kind: key.kind, deviceID: connected.id)] = controller
            }
            if let previous = lastDevice, previous.id != connected.id {
                let kinds = windows.keys.filter { $0.deviceID == previous.id }.map(\.kind)
                for key in windows.keys where key.deviceID == previous.id { windows[key]?.close() }
                nativeSettings[previous.id]?.close()
                nativeSettings[previous.id] = nil
                lastDevice = connected
                for kind in DeviceWindowKind.allCases where kinds.contains(kind) { show(kind) }
            }
            lastDevice = connected
        }
        for (key, controller) in windows {
            controller.update(device: device(for: key.deviceID), connectedDevice: connected)
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
        let controller = kind.flatMap { kind in windows.first { $0.key.kind == kind }?.value }
        switch verb {
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

    /// The open windows, for the test hook.
    var windowList: [[String: Any]] {
        windows.map { key, controller in
            let frame = controller.window?.frame ?? .zero
            return [
                "kind": key.kind.rawValue,
                "device": key.deviceID ?? "",
                "visible": controller.window?.isVisible ?? false,
                "minimized": controller.window?.isMiniaturized ?? false,
                "subtitle": controller.window?.subtitle ?? "",
                "window_number": controller.window?.windowNumber ?? 0,
                "frame": [frame.origin.x, frame.origin.y, frame.width, frame.height],
            ]
        } + nativeSettings.map { id, controller in
            ["kind": "native-system-settings", "device": id, "visible": controller.window?.isVisible ?? false,
             "window_number": controller.window?.windowNumber ?? 0]
        }
    }

    func deviceWindow(_ webView: WKWebView?, didRequest name: String, body: Any) {
        switch name {
        case "openSystemSettings": showSystemSettings()
        case "openPresets": show(.presets)
        case "openMeters": show(.meters)
        case "openProcessing": show(.processing)
        default: appDelegate?.deviceWindow(webView, didRequest: name, body: body)
        }
    }
}
