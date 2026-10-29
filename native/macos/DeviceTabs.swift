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

    private let scroll = NSScrollView()
    private let strip = NSView()
    private var tabViews: [DeviceTabView] = []
    private let addButton = NSButton()
    private var shown: ([DeviceTab], String?) = ([], nil)

    /// Tabs share the width, between these sizes; below the minimum the strip
    /// scrolls sideways and the + button stays in view.
    static let tabWidths: ClosedRange<CGFloat> = 112...300
    static let leading: CGFloat = 8
    static let addWidth: CGFloat = 36

    /// The width of each of `count` tabs in a bar `width` wide.
    static func tabWidth(count: Int, width: CGFloat) -> CGFloat {
        guard count > 0 else { return tabWidths.upperBound }
        let available = width - leading - addWidth - 2 * CGFloat(count - 1)
        return min(max(available / CGFloat(count), tabWidths.lowerBound), tabWidths.upperBound)
    }

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = Self.background.cgColor
        scroll.drawsBackground = false
        scroll.hasHorizontalScroller = false
        scroll.hasVerticalScroller = false
        scroll.horizontalScrollElasticity = .allowed
        scroll.verticalScrollElasticity = .none
        scroll.documentView = strip
        scroll.translatesAutoresizingMaskIntoConstraints = false
        addButton.image = NSImage(systemSymbolName: "plus", accessibilityDescription: "Connect another processor")
        addButton.isBordered = false
        addButton.contentTintColor = NSColor(white: 0.75, alpha: 1)
        addButton.target = self
        addButton.action = #selector(add)
        addButton.toolTip = "Connect another processor"
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scroll)
        addSubview(addButton)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: Self.height),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Self.leading),
            scroll.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            scrollWidth,
            addButton.leadingAnchor.constraint(equalTo: scroll.trailingAnchor, constant: 6),
            addButton.centerYAnchor.constraint(equalTo: scroll.centerYAnchor),
            addButton.widthAnchor.constraint(equalToConstant: 24),
            addButton.heightAnchor.constraint(equalToConstant: 24),
            addButton.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -6),
        ])
        setAccessibilityRole(.tabGroup)
        setAccessibilityLabel("Processors")
    }

    private lazy var scrollWidth = scroll.widthAnchor.constraint(equalToConstant: 0)

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func update(tabs: [DeviceTab], selected: String?) {
        guard shown.0 != tabs || shown.1 != selected else { return }
        shown = (tabs, selected)
        tabViews.forEach { $0.removeFromSuperview() }
        tabViews = tabs.map { tab in
            let view = DeviceTabView(tab: tab, selected: tab.id == selected)
            view.onSelect = { [weak self] in self?.onSelect?(tab.id) }
            view.onClose = { [weak self] in self?.onClose?(tab.id) }
            strip.addSubview(view)
            return view
        }
        needsLayout = true
    }

    override func layout() {
        let width = Self.tabWidth(count: tabViews.count, width: bounds.width)
        let content = CGFloat(tabViews.count) * width + 2 * CGFloat(max(tabViews.count - 1, 0))
        let room = max(bounds.width - Self.leading - Self.addWidth, 0)
        // Set before the constraints are solved: a constant changed after
        // super.layout() waits for some later pass, and until then a tab added
        // to a window that does not resize stays in a strip of zero width.
        scrollWidth.constant = min(content, room)
        super.layout()
        let height = Self.height - 4
        strip.frame = NSRect(x: 0, y: 0, width: content, height: height)
        for (index, view) in tabViews.enumerated() {
            view.frame = NSRect(x: CGFloat(index) * (width + 2), y: 0, width: width, height: 30)
        }
        // The selected tab stays in view when the strip scrolls.
        if let selected = tabViews.first(where: \.isSelected) { strip.scrollToVisible(selected.frame) }
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
        focusRingType = .exterior

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

    var isSelected: Bool { selected }

    // Full keyboard access: Tab reaches each tab, Space or Return chooses it,
    // Delete closes it.
    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { true }
    override var focusRingMaskBounds: NSRect { bounds }
    override func drawFocusRingMask() { NSBezierPath(roundedRect: bounds, xRadius: 6, yRadius: 6).fill() }

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 36, 49, 76: onSelect?()
        case 51, 117: onClose?()
        default: super.keyDown(with: event)
        }
    }

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
