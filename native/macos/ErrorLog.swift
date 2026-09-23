import AppKit
import Foundation

struct OptimodErrorEntry: Codable, Equatable {
    let date: Date
    let source: String
    let message: String
}

extension Notification.Name {
    static let optimodErrorLogChanged = Notification.Name("OpenOptimodRemote.ErrorLogChanged")
}

final class ErrorLogStore {
    static let shared = ErrorLogStore()

    private(set) var entries: [OptimodErrorEntry] = []
    private let fileURL: URL

    private init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("OpenOptimodRemote", isDirectory: true)
        try? FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        fileURL = base.appendingPathComponent("errors.json")
        if let data = try? Data(contentsOf: fileURL),
           let decoded = try? JSONDecoder().decode([OptimodErrorEntry].self, from: data) {
            entries = Array(decoded.suffix(500))
        }
    }

    func record(source: String, message: String) {
        dispatchPrecondition(condition: .onQueue(.main))
        let cleanSource = source.trimmingCharacters(in: .whitespacesAndNewlines)
        let cleanMessage = Self.redact(message.trimmingCharacters(in: .whitespacesAndNewlines))
        guard !cleanMessage.isEmpty else { return }
        if entries.last.map({ $0.source == cleanSource && $0.message == cleanMessage }) == true { return }
        entries.append(OptimodErrorEntry(date: Date(), source: cleanSource.isEmpty ? "Open Optimod Remote" : cleanSource, message: cleanMessage))
        if entries.count > 500 { entries.removeFirst(entries.count - 500) }
        persist()
        NotificationCenter.default.post(name: .optimodErrorLogChanged, object: self)
    }

    func clear() {
        dispatchPrecondition(condition: .onQueue(.main))
        entries.removeAll()
        persist()
        NotificationCenter.default.post(name: .optimodErrorLogChanged, object: self)
    }

    private func persist() {
        guard let data = try? JSONEncoder().encode(entries) else { return }
        try? data.write(to: fileURL, options: .atomic)
    }

    static func redact(_ value: String) -> String {
        let pattern = #"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3})\b"#
        guard let expression = try? NSRegularExpression(pattern: pattern) else { return value }
        return expression.stringByReplacingMatches(
            in: value,
            range: NSRange(value.startIndex..., in: value),
            withTemplate: "[local Optimod]"
        )
    }
}

final class ErrorLogWindowController: NSWindowController, NSTableViewDataSource, NSTableViewDelegate, NSToolbarDelegate {
    private let tableView = NSTableView()
    private let emptyLabel = NSTextField(labelWithString: "No errors have been recorded.")

    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 780, height: 480),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Error Log"
        window.minSize = NSSize(width: 620, height: 360)
        super.init(window: window)
        configure()
        window.center()
        NotificationCenter.default.addObserver(self, selector: #selector(reload), name: .optimodErrorLogChanged, object: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    deinit { NotificationCenter.default.removeObserver(self) }

    func show() {
        reload()
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    private func configure() {
        guard let window else { return }
        let toolbar = NSToolbar(identifier: "OpenOptimodErrorLogToolbar")
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        window.toolbar = toolbar
        window.toolbarStyle = .unified

        let time = NSTableColumn(identifier: .init("time"))
        time.title = "Time"
        time.width = 150
        let source = NSTableColumn(identifier: .init("source"))
        source.title = "Source"
        source.width = 150
        let message = NSTableColumn(identifier: .init("message"))
        message.title = "Message"
        message.resizingMask = .autoresizingMask
        [time, source, message].forEach(tableView.addTableColumn)
        tableView.delegate = self
        tableView.dataSource = self
        tableView.rowHeight = 34
        tableView.usesAlternatingRowBackgroundColors = true

        let scroll = NSScrollView()
        scroll.documentView = tableView
        scroll.hasVerticalScroller = true
        scroll.translatesAutoresizingMaskIntoConstraints = false
        emptyLabel.textColor = .secondaryLabelColor
        emptyLabel.font = .systemFont(ofSize: 15)
        emptyLabel.translatesAutoresizingMaskIntoConstraints = false
        let content = NSView()
        content.addSubview(scroll)
        content.addSubview(emptyLabel)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: content.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: content.bottomAnchor),
            emptyLabel.centerXAnchor.constraint(equalTo: content.centerXAnchor),
            emptyLabel.centerYAnchor.constraint(equalTo: content.centerYAnchor),
        ])
        window.contentView = content
        reload()
    }

    @objc private func reload() {
        tableView.reloadData()
        emptyLabel.isHidden = !ErrorLogStore.shared.entries.isEmpty
    }

    func numberOfRows(in tableView: NSTableView) -> Int { ErrorLogStore.shared.entries.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard ErrorLogStore.shared.entries.indices.contains(row), let tableColumn else { return nil }
        let entry = ErrorLogStore.shared.entries[row]
        let value: String
        switch tableColumn.identifier.rawValue {
        case "time": value = Self.dateFormatter.string(from: entry.date)
        case "source": value = entry.source
        default: value = entry.message
        }
        let field = NSTextField(labelWithString: value)
        field.lineBreakMode = .byTruncatingTail
        field.toolTip = value
        return field
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { [.clear, .flexibleSpace] }
    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { [.flexibleSpace, .clear] }
    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier id: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        guard id == .clear else { return nil }
        let item = NSToolbarItem(itemIdentifier: id)
        item.label = "Clear"
        item.paletteLabel = "Clear Error Log"
        item.image = NSImage(systemSymbolName: "trash", accessibilityDescription: "Clear Error Log")
        item.target = self
        item.action = #selector(clear)
        return item
    }

    @objc private func clear() { ErrorLogStore.shared.clear() }

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .medium
        return formatter
    }()
}

private extension NSToolbarItem.Identifier {
    static let clear = NSToolbarItem.Identifier("OpenOptimodErrorLogClear")
}
