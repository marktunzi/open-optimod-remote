import AppKit

/// The processor tabs above Meters and Processing Settings, as Figma's file
/// tabs: one tab per open processor session. Choosing a tab only changes what
/// the windows show; every other processor stays connected.
final class DeviceTabBar: NSView {
    var onSelect: ((String) -> Void)?
    var onClose: ((String) -> Void)?
    var onAdd: (() -> Void)?

    static let height: CGFloat = 34
    /// Also the title bar above the tabs.
    static let background = NSColor(srgbRed: 0x2c / 255, green: 0x2c / 255, blue: 0x2c / 255, alpha: 1)

    private let stack = NSStackView()
    private let addButton = NSButton()
    private var shown: ([DeviceTab], String?) = ([], nil)

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = Self.background.cgColor
        stack.orientation = .horizontal
        stack.spacing = 2
        stack.alignment = .bottom
        stack.translatesAutoresizingMaskIntoConstraints = false
        addButton.image = NSImage(systemSymbolName: "plus", accessibilityDescription: "Connect another processor")
        addButton.isBordered = false
        addButton.contentTintColor = NSColor(white: 0.75, alpha: 1)
        addButton.target = self
        addButton.action = #selector(add)
        addButton.toolTip = "Connect another processor"
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        addSubview(addButton)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: Self.height),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            addButton.leadingAnchor.constraint(equalTo: stack.trailingAnchor, constant: 6),
            addButton.centerYAnchor.constraint(equalTo: stack.centerYAnchor),
            addButton.widthAnchor.constraint(equalToConstant: 24),
            addButton.heightAnchor.constraint(equalToConstant: 24),
            addButton.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
        ])
        setAccessibilityRole(.tabGroup)
        setAccessibilityLabel("Processors")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func update(tabs: [DeviceTab], selected: String?) {
        guard shown.0 != tabs || shown.1 != selected else { return }
        shown = (tabs, selected)
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for tab in tabs {
            let view = DeviceTabView(tab: tab, selected: tab.id == selected)
            view.onSelect = { [weak self] in self?.onSelect?(tab.id) }
            view.onClose = { [weak self] in self?.onClose?(tab.id) }
            stack.addArrangedSubview(view)
        }
    }

    @objc private func add() { onAdd?() }
}

/// One processor tab: its name, model and connection state, with a close
/// button. The selected tab takes the window's colour, as in Figma.
final class DeviceTabView: NSView {
    var onSelect: (() -> Void)?
    var onClose: (() -> Void)?

    private static let selectedFill = NSColor(srgbRed: 0x4c / 255, green: 0x4c / 255, blue: 0x4c / 255, alpha: 1)
    private static let hoverFill = NSColor(srgbRed: 0x38 / 255, green: 0x38 / 255, blue: 0x38 / 255, alpha: 1)

    private let tab: DeviceTab
    private let selected: Bool
    private let close = NSButton()
    private var hovering = false { didSet { refresh() } }

    init(tab: DeviceTab, selected: Bool) {
        self.tab = tab
        self.selected = selected
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        translatesAutoresizingMaskIntoConstraints = false

        let dot = NSView()
        dot.wantsLayer = true
        dot.layer?.cornerRadius = 3.5
        dot.layer?.backgroundColor = Self.color(tab.status).cgColor
        dot.translatesAutoresizingMaskIntoConstraints = false

        let title = NSTextField(labelWithString: tab.title)
        title.font = .systemFont(ofSize: 12, weight: selected ? .semibold : .regular)
        title.textColor = selected ? .white : NSColor(white: 0.72, alpha: 1)
        title.lineBreakMode = .byTruncatingTail
        let detail = NSTextField(labelWithString: tab.detail)
        detail.font = .systemFont(ofSize: 11)
        detail.textColor = NSColor(white: selected ? 0.72 : 0.55, alpha: 1)
        detail.lineBreakMode = .byTruncatingTail

        close.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close \(tab.title)")
        close.isBordered = false
        close.imageScaling = .scaleProportionallyDown
        close.contentTintColor = NSColor(white: 0.75, alpha: 1)
        close.target = self
        close.action = #selector(closeTab)
        close.toolTip = tab.status == .lost ? "Close tab" : "Disconnect \(tab.title)"
        close.translatesAutoresizingMaskIntoConstraints = false

        let labels = NSStackView(views: [title, detail])
        labels.orientation = .horizontal
        labels.spacing = 6
        labels.translatesAutoresizingMaskIntoConstraints = false
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        detail.setContentCompressionResistancePriority(.defaultLow - 1, for: .horizontal)
        for view in [dot, labels, close] { addSubview(view) }
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 30),
            widthAnchor.constraint(lessThanOrEqualToConstant: 280),
            widthAnchor.constraint(greaterThanOrEqualToConstant: 120),
            dot.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            dot.centerYAnchor.constraint(equalTo: centerYAnchor),
            dot.widthAnchor.constraint(equalToConstant: 7),
            dot.heightAnchor.constraint(equalToConstant: 7),
            labels.leadingAnchor.constraint(equalTo: dot.trailingAnchor, constant: 8),
            labels.centerYAnchor.constraint(equalTo: centerYAnchor),
            close.leadingAnchor.constraint(equalTo: labels.trailingAnchor, constant: 8),
            close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            close.centerYAnchor.constraint(equalTo: centerYAnchor),
            close.widthAnchor.constraint(equalToConstant: 16),
            close.heightAnchor.constraint(equalToConstant: 16),
        ])
        toolTip = "\(tab.title) · \(tab.detail) · \(Self.statusText(tab.status))"
        setAccessibilityElement(true)
        setAccessibilityRole(.radioButton)
        setAccessibilityLabel("\(tab.title), \(tab.detail), \(Self.statusText(tab.status))")
        setAccessibilityValue(selected)
        refresh()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    static func color(_ status: DeviceTab.Status) -> NSColor {
        switch status {
        case .connected: NSColor(srgbRed: 0x14 / 255, green: 0xae / 255, blue: 0x5c / 255, alpha: 1)
        case .reconnecting: NSColor(srgbRed: 0xff / 255, green: 0xa6 / 255, blue: 0x29 / 255, alpha: 1)
        case .lost: NSColor(srgbRed: 0xf2 / 255, green: 0x48 / 255, blue: 0x22 / 255, alpha: 1)
        }
    }

    static func statusText(_ status: DeviceTab.Status) -> String {
        switch status {
        case .connected: "Connected"
        case .reconnecting: "Reconnecting"
        case .lost: "Connection lost"
        }
    }

    private func refresh() {
        layer?.backgroundColor = (selected ? Self.selectedFill : hovering ? Self.hoverFill : .clear).cgColor
        close.isHidden = !(selected || hovering)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect], owner: self))
    }

    override func mouseEntered(with event: NSEvent) { hovering = true }
    override func mouseExited(with event: NSEvent) { hovering = false }
    override func mouseDown(with event: NSEvent) { onSelect?() }
    override func accessibilityPerformPress() -> Bool {
        onSelect?()
        return true
    }

    @objc private func closeTab() { onClose?() }
}
