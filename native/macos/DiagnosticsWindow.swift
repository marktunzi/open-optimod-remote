import AppKit

/// The processor's own status text (opcode 205 after the PC Remote login),
/// as the original client's Diagnostics dialog shows it.
final class DiagnosticsWindowController: NSWindowController {
    private let textView = NSTextView()
    /// The processor whose status text this shows: one window per processor.
    let deviceID: String?

    init(deviceID: String?) {
        self.deviceID = deviceID
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 520, height: 300),
                              styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = "Diagnostics"
        window.isReleasedWhenClosed = false
        window.setFrameAutosaveName("OpenOptimod.Diagnostics")
        textView.isEditable = false
        textView.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        textView.textContainerInset = NSSize(width: 12, height: 12)
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.documentView = textView
        textView.autoresizingMask = [.width]
        window.contentView = scroll
        super.init(window: window)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func show() {
        window?.center()
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        reload()
    }

    private func reload() {
        textView.string = "Reading…"
        let request = URLRequest(url: BackendHealth.api("state", device: deviceID), cachePolicy: .reloadIgnoringLocalCacheData)
        URLSession.shared.dataTask(with: request) { [weak self] data, _, error in
            let state = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
            let text: String
            if let error {
                text = error.localizedDescription
            } else if state?["connected"] as? Bool != true {
                text = "Connect to a processor first."
            } else if let diagnostics = state?["diagnostics"] as? String, !diagnostics.isEmpty {
                text = diagnostics
            } else {
                text = "This processor sent no diagnostics."
            }
            DispatchQueue.main.async {
                self?.textView.string = text
                if let host = state?["host"] as? String, !host.isEmpty { self?.window?.title = "Diagnostics – \(host)" }
            }
        }.resume()
    }
}
