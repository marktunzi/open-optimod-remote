import AppKit
import Foundation
import UniformTypeIdentifiers

final class PresetsWindowController: NSWindowController, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate, NSToolbarDelegate {
    private let api = PresetsAPI()
    private let tableView = NSTableView()
    private let splitViewController = NSSplitViewController()
    private var searchField = NSSearchField()
    private let titleLabel = NSTextField(labelWithString: "All Presets")
    private let statusLabel = NSTextField(labelWithString: "Loading presets…")
    private let recallButton = NSButton(title: "Recall Preset", target: nil, action: nil)
    private let refreshButton = NSButton()
    private let saveButton = NSButton()
    private let applyButton = NSButton()
    private let actionsButton = NSButton()
    private let progress = NSProgressIndicator()
    private var filterButtons: [PresetBrowserFilter: PresetSidebarSourceButton] = [:]
    private var context: PresetBrowserContext?
    private var visible: [DevicePresetPayload] = []
    private var selectedName: String?
    private var filter = PresetBrowserFilter.all
    private var busy = false
    var onChanged: (() -> Void)?

    init() {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: PresetPresentation.defaultSize),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Presets"
        window.contentMinSize = PresetPresentation.minimumSize
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.toolbarStyle = .unified
        super.init(window: window)
        configureWindow()
        window.setFrameAutosaveName("OpenOptimodRemotePresetsWindowV3")
        window.center()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func show() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        reload()
    }

    private func configureWindow() {
        guard let window else { return }
        let toolbar = NSToolbar(identifier: "OpenOptimodPresetsToolbar")
        toolbar.delegate = self
        // The custom toolbar controls already contain both icon and text.
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        window.toolbar = toolbar
        let sidebarController = NSViewController()
        sidebarController.view = makeSidebar()
        let contentController = NSViewController()
        contentController.view = makeContent()
        let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebarController)
        sidebarItem.minimumThickness = PresetPresentation.minimumSidebarWidth
        sidebarItem.maximumThickness = PresetPresentation.maximumSidebarWidth
        sidebarItem.automaticMaximumThickness = 220
        sidebarItem.preferredThicknessFraction = PresetPresentation.sidebarWidth / PresetPresentation.defaultSize.width
        sidebarItem.canCollapse = true
        sidebarItem.allowsFullHeightLayout = true
        sidebarItem.titlebarSeparatorStyle = .none
        let contentItem = NSSplitViewItem(viewController: contentController)
        contentItem.titlebarSeparatorStyle = .none
        splitViewController.addSplitViewItem(sidebarItem)
        splitViewController.addSplitViewItem(contentItem)
        splitViewController.splitView.dividerStyle = .thin
        window.contentViewController = splitViewController
        window.layoutIfNeeded()
        splitViewController.splitView.setPosition(PresetPresentation.sidebarWidth, ofDividerAt: 0)
    }

    private func makeSidebar() -> NSView {
        let sidebar = NSVisualEffectView()
        sidebar.material = .sidebar
        sidebar.blendingMode = .behindWindow
        sidebar.state = .followsWindowActiveState
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        for category in PresetBrowserFilter.allCases {
            let button = PresetSidebarSourceButton(title: category.rawValue, target: self, action: #selector(selectFilter(_:)))
            button.identifier = NSUserInterfaceItemIdentifier(category.rawValue)
            button.image = NSImage(systemSymbolName: symbol(for: category), accessibilityDescription: category.rawValue)
            button.imagePosition = .imageLeading
            button.alignment = .left
            button.setButtonType(.toggle)
            button.isBordered = false
            button.font = .systemFont(ofSize: 13)
            button.contentTintColor = .labelColor
            button.state = category == .all ? .on : .off
            stack.addArrangedSubview(button)
            button.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
            button.heightAnchor.constraint(equalToConstant: 34).isActive = true
            filterButtons[category] = button
        }
        sidebar.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -12),
            stack.topAnchor.constraint(equalTo: sidebar.topAnchor, constant: 54),
        ])
        return sidebar
    }

    private func symbol(for filter: PresetBrowserFilter) -> String {
        switch filter {
        case .all: return "square.stack.3d.up"
        case .factory: return "building.columns"
        case .user: return "person.crop.circle"
        case .modified: return "slider.horizontal.3"
        }
    }

    private func makeContent() -> NSView {
        let content = NSView()
        statusLabel.textColor = .secondaryLabelColor
        statusLabel.font = .systemFont(ofSize: 12)

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("preset"))
        column.resizingMask = .autoresizingMask
        tableView.addTableColumn(column)
        tableView.headerView = nil
        tableView.rowHeight = PresetPresentation.rowHeight
        tableView.intercellSpacing = NSSize(width: 0, height: 1)
        tableView.style = .fullWidth
        tableView.allowsEmptySelection = true
        tableView.delegate = self
        tableView.dataSource = self
        tableView.target = self
        tableView.doubleAction = nil
        let scroll = NSScrollView()
        scroll.documentView = tableView
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        progress.style = .spinning
        progress.controlSize = .small
        progress.isDisplayedWhenStopped = false
        progress.translatesAutoresizingMaskIntoConstraints = false

        for view in [scroll, progress] { content.addSubview(view) }
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
            scroll.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
            scroll.topAnchor.constraint(equalTo: content.topAnchor, constant: 14),
            scroll.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -16),
            progress.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
            progress.topAnchor.constraint(equalTo: content.topAnchor, constant: 16),
        ])
        return content
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.recallPreset, .presetActions, .refreshPresets, .flexibleSpace, .presetSearch]
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.recallPreset, .flexibleSpace, .refreshPresets, .presetActions, .presetSearch]
    }

    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier id: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        switch id {
        case .recallPreset:
            return toolbarButton(id: id, button: recallButton, symbol: "play.fill", label: "Recall", action: #selector(recallSelected))
        case .refreshPresets:
            return iconToolbarButton(id: id, button: refreshButton, symbol: "arrow.clockwise", label: "Refresh", action: #selector(refreshPresets))
        case .presetActions:
            return iconToolbarButton(id: id, button: actionsButton, symbol: "ellipsis.circle", label: "Preset Actions", action: #selector(showPresetActions(_:)))
        case .presetSearch:
            let item = NSSearchToolbarItem(itemIdentifier: id)
            item.label = "Search"
            item.searchField.placeholderString = "Search"
            item.searchField.delegate = self
            item.searchField.sendsSearchStringImmediately = true
            searchField = item.searchField
            return item
        default: return nil
        }
    }

    private func toolbarButton(id: NSToolbarItem.Identifier, button: NSButton, symbol: String, label: String, action: Selector) -> NSToolbarItem {
        button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
        button.title = label
        button.imagePosition = .imageLeading
        button.font = .systemFont(ofSize: 13, weight: id == .recallPreset ? .medium : .regular)
        button.bezelStyle = .texturedRounded
        button.target = self
        button.action = action
        button.toolTip = label
        let item = NSToolbarItem(itemIdentifier: id)
        item.label = label
        item.view = button
        item.visibilityPriority = id == .recallPreset ? .high : .standard
        button.widthAnchor.constraint(greaterThanOrEqualToConstant: id == .recallPreset ? 86 : 98).isActive = true
        if id == .recallPreset { button.keyEquivalent = "\r" }
        return item
    }

    private func iconToolbarButton(id: NSToolbarItem.Identifier, button: NSButton, symbol: String, label: String, action: Selector) -> NSToolbarItem {
        button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
        button.title = ""
        button.bezelStyle = .texturedRounded
        button.target = self
        button.action = action
        button.toolTip = label
        button.widthAnchor.constraint(equalToConstant: 32).isActive = true
        button.heightAnchor.constraint(equalToConstant: 28).isActive = true
        let item = NSToolbarItem(itemIdentifier: id)
        item.label = label
        item.view = button
        return item
    }

    @objc private func showPresetActions(_ sender: NSButton) {
        let menu = NSMenu()
        let apply = menu.addItem(withTitle: "Apply Preset File…", action: #selector(applyPresetFile), keyEquivalent: "o")
        apply.target = self
        apply.isEnabled = !busy && context?.snapshot.connected == true && context?.snapshot.writeEnabled == true
        let save = menu.addItem(withTitle: "Save Current Preset to Mac…", action: #selector(savePresetFile), keyEquivalent: "S")
        save.target = self
        save.isEnabled = !busy && context?.snapshot.connected == true
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: sender.bounds.height + 4), in: sender)
    }

    @objc private func selectFilter(_ sender: NSButton) {
        guard let value = sender.identifier?.rawValue,
              let selected = PresetBrowserFilter(rawValue: value) else { return }
        filter = selected
        titleLabel.stringValue = selected.rawValue
        for (category, button) in filterButtons { button.state = category == selected ? .on : .off }
        applyFilter()
    }

    @objc private func refreshPresets() { reload() }

    private func reload() {
        guard !busy else { return }
        setBusy(true, message: "Loading presets…")
        api.load { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error):
                self.setBusy(false, message: "Presets unavailable")
                self.presentError(error.localizedDescription)
            case let .success(context):
                self.context = context
                self.applyFilter()
                let message = context.snapshot.connected
                    ? "On air: \(PresetPresentation.displayName(context.currentName))"
                    : "Connect to an Optimod to load presets"
                self.setBusy(false, message: message)
            }
        }
    }

    private func applyFilter() {
        let query = searchField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        visible = (context?.snapshot.presets ?? []).filter {
            filter.includes($0) && (query.isEmpty || $0.name.localizedStandardContains(query))
        }
        tableView.reloadData()
        if let selectedName, let row = visible.firstIndex(where: { $0.name == selectedName }) {
            tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
        } else { tableView.deselectAll(nil) }
        updateRecallButton()
    }

    private func setBusy(_ value: Bool, message: String) {
        busy = value
        statusLabel.stringValue = message
        if value { progress.startAnimation(nil) } else { progress.stopAnimation(nil) }
        refreshButton.isEnabled = !value
        actionsButton.isEnabled = !value
        saveButton.isEnabled = !value && context?.snapshot.connected == true
        applyButton.isEnabled = !value && context?.snapshot.connected == true && context?.snapshot.writeEnabled == true
        searchField.isEnabled = !value
        for button in filterButtons.values { button.isEnabled = !value }
        updateRecallButton()
    }

    func numberOfRows(in tableView: NSTableView) -> Int { visible.count }

    func tableView(_ tableView: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
        PresetTableRowView()
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard visible.indices.contains(row) else { return nil }
        let preset = visible[row]
        let cell = NSTableCellView()
        let icon = NSImageView(image: NSImage(systemSymbolName: preset.kind == "Factory" ? "building.columns" : preset.kind == "User" ? "person.crop.circle" : "slider.horizontal.3", accessibilityDescription: nil) ?? NSImage())
        icon.contentTintColor = .secondaryLabelColor
        icon.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 15, weight: .regular)
        let name = NSTextField(labelWithString: PresetPresentation.displayName(preset.name))
        name.font = .systemFont(ofSize: 13, weight: .medium)
        name.lineBreakMode = .byTruncatingTail
        let kind = NSTextField(labelWithString: preset.kind == "Unsaved" ? "Modified, not saved" : preset.kind)
        kind.textColor = .secondaryLabelColor
        kind.font = .systemFont(ofSize: 11)
        let labels = NSStackView(views: [name, kind])
        labels.orientation = .vertical
        labels.alignment = .leading
        labels.spacing = 2
        let onAir = NSTextField(labelWithString: preset.name == context?.currentName ? "On Air" : "")
        onAir.textColor = .systemGreen
        onAir.font = .systemFont(ofSize: 11, weight: .medium)
        let onAirIcon = NSImageView(image: NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "On Air") ?? NSImage())
        onAirIcon.contentTintColor = .systemGreen
        onAirIcon.isHidden = preset.name != context?.currentName
        let state = NSStackView(views: [onAirIcon, onAir])
        state.orientation = .horizontal
        state.alignment = .centerY
        state.spacing = 4
        let rowView = NSStackView(views: [icon, labels, NSView(), state])
        rowView.orientation = .horizontal
        rowView.alignment = .centerY
        rowView.spacing = 12
        rowView.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(rowView)
        NSLayoutConstraint.activate([
            rowView.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 10),
            rowView.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -12),
            rowView.topAnchor.constraint(equalTo: cell.topAnchor, constant: 3),
            rowView.bottomAnchor.constraint(equalTo: cell.bottomAnchor, constant: -3),
            icon.widthAnchor.constraint(equalToConstant: 18),
            icon.heightAnchor.constraint(equalToConstant: 18),
            onAirIcon.widthAnchor.constraint(equalToConstant: 14),
            onAirIcon.heightAnchor.constraint(equalToConstant: 14),
        ])
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        let row = tableView.selectedRow
        selectedName = visible.indices.contains(row) ? visible[row].name : nil
        updateRecallButton()
    }

    func controlTextDidChange(_ obj: Notification) { applyFilter() }

    private func updateRecallButton() {
        let preset = visible[safe: tableView.selectedRow]
        recallButton.isEnabled = !busy
            && context?.snapshot.connected == true
            && context?.snapshot.writeEnabled == true
            && preset != nil
            && preset?.kind != "Unsaved"
            && preset?.name != context?.currentName
    }

    @objc private func recallSelected() {
        guard !busy, let context, let session = context.snapshot.sessionID,
              let preset = visible[safe: tableView.selectedRow], preset.kind != "Unsaved",
              preset.name != context.currentName else { return }
        let request = PresetRecallRequest(
            name: preset.name,
            expectedName: context.currentName,
            confirmed: true,
            expectedHost: context.snapshot.host,
            expectedSession: session
        )
        setBusy(true, message: "Recalling \(PresetPresentation.displayName(preset.name))…")
        api.recall(request) { [weak self] result in
            guard let self else { return }
            self.onChanged?()
            switch result {
            case let .failure(error):
                self.setBusy(false, message: "Recall failed")
                self.presentError(error.localizedDescription)
                self.reload()
            case .success:
                self.selectedName = preset.name
                self.setBusy(false, message: "\(PresetPresentation.displayName(preset.name)) is on air")
                self.reload()
            }
        }
    }

    @objc private func savePresetFile() {
        guard !busy, context?.snapshot.connected == true, let window else { return }
        setBusy(true, message: "Reading current processing…")
        api.currentFile { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error):
                self.setBusy(false, message: "Save failed")
                self.presentError(error.localizedDescription)
            case let .success(file):
                self.setBusy(false, message: "Choose where to save the preset")
                let panel = NSSavePanel()
                panel.title = "Save Current OPTIMOD Preset"
                panel.nameFieldStringValue = self.safeFilename(file.name) + ".orb57user"
                panel.allowedContentTypes = [UTType(filenameExtension: "orb57user") ?? .data]
                panel.canCreateDirectories = true
                panel.beginSheetModal(for: window) { response in
                    guard response == .OK, let url = panel.url else { return }
                    do {
                        try file.document.write(to: url, atomically: true, encoding: .utf8)
                        self.statusLabel.stringValue = "Saved (url.lastPathComponent)"
                    } catch {
                        self.presentError(error.localizedDescription)
                    }
                }
            }
        }
    }

    @objc private func applyPresetFile() {
        guard !busy, let context, context.snapshot.connected, context.snapshot.writeEnabled,
              let session = context.snapshot.sessionID, let window else { return }
        let panel = NSOpenPanel()
        panel.title = "Choose an OPTIMOD 5700i Preset"
        panel.allowedContentTypes = [
            UTType(filenameExtension: "orb57user") ?? .data,
            UTType(filenameExtension: "orb") ?? .data,
            .plainText,
        ]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let self, let url = panel.url else { return }
            do {
                let data = try Data(contentsOf: url, options: [.mappedIfSafe])
                guard data.count <= 65_535, let document = String(data: data, encoding: .utf8) else {
                    throw SystemSettingsAPIError.service("This is not a supported OPTIMOD 5700i preset file.")
                }
                let alert = NSAlert()
                alert.alertStyle = .warning
                alert.messageText = "Apply (url.lastPathComponent)?"
                alert.informativeText = "All processing values in this file will be written to the connected OPTIMOD and verified."
                alert.addButton(withTitle: "Apply Preset")
                alert.addButton(withTitle: "Cancel")
                alert.beginSheetModal(for: window) { answer in
                    guard answer == .alertFirstButtonReturn else { return }
                    let request = PresetApplyRequest(
                        document: document,
                        expectedName: context.currentName,
                        confirmed: true,
                        expectedHost: context.snapshot.host,
                        expectedSession: session
                    )
                    self.setBusy(true, message: "Applying and verifying (url.lastPathComponent)…")
                    self.api.apply(request) { result in
                        switch result {
                        case let .failure(error):
                            self.setBusy(false, message: "Apply failed")
                            self.presentError(error.localizedDescription)
                        case .success:
                            self.onChanged?()
                            self.setBusy(false, message: "Preset file applied")
                            self.reload()
                        }
                    }
                }
            } catch {
                self.presentError(error.localizedDescription)
            }
        }
    }

    private func safeFilename(_ value: String) -> String {
        let invalid = CharacterSet(charactersIn: "/\\:[]").union(.controlCharacters)
        let clean = value.components(separatedBy: invalid).joined(separator: "-")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return clean.isEmpty ? "OPTIMOD Preset" : clean
    }

    private func presentError(_ message: String) {
        ErrorLogStore.shared.record(source: "Presets", message: message)
        statusLabel.stringValue = "Operation failed · See Error Log"
        NSSound.beep()
    }
}

private final class PresetSidebarSourceButton: NSButton {
    override var state: NSControl.StateValue {
        didSet {
            contentTintColor = .labelColor
            needsDisplay = true
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        if state == .on {
            let color = window?.isKeyWindow == true
                ? NSColor.selectedContentBackgroundColor.withAlphaComponent(0.28)
                : NSColor.unemphasizedSelectedContentBackgroundColor
            color.setFill()
            NSBezierPath(roundedRect: bounds.insetBy(dx: 0, dy: 1), xRadius: 8, yRadius: 8).fill()
        }
        super.draw(dirtyRect)
    }
}

private final class PresetTableRowView: NSTableRowView {
    override func drawSelection(in dirtyRect: NSRect) {
        super.drawSelection(in: dirtyRect)
    }
}

enum PresetPresentation {
    static let defaultSize = NSSize(width: 900, height: 560)
    static let minimumSize = NSSize(width: 760, height: 460)
    static let rowHeight: CGFloat = 40
    static let sidebarWidth: CGFloat = 188
    static let minimumSidebarWidth: CGFloat = 168
    static let maximumSidebarWidth: CGFloat = 240
    private static let acronyms: Set<String> = ["AC", "AGC", "CHR", "EQ", "FM", "HD", "IBOC", "LL", "R&B", "UL", "2B", "5B"]

    static func displayName(_ raw: String) -> String {
        raw.split(separator: " ", omittingEmptySubsequences: false)
            .map { formatCompound(String($0)) }
            .joined(separator: " ")
    }

    private static func formatCompound(_ value: String) -> String {
        var result = ""
        var token = ""
        func formatted(_ token: String) -> String {
            guard !token.isEmpty else { return token }
            if acronyms.contains(token) || token.allSatisfy(\.isNumber) { return token }
            return token.prefix(1).uppercased() + token.dropFirst().lowercased()
        }
        for character in value {
            if character == "-" || character == "+" {
                result += formatted(token) + String(character)
                token = ""
            } else {
                token.append(character)
            }
        }
        return result + formatted(token)
    }
}

private extension NSToolbarItem.Identifier {
    static let recallPreset = NSToolbarItem.Identifier("OpenOptimodRecallPreset")
    static let applyPresetFile = NSToolbarItem.Identifier("OpenOptimodApplyPresetFile")
    static let savePresetFile = NSToolbarItem.Identifier("OpenOptimodSavePresetFile")
    static let refreshPresets = NSToolbarItem.Identifier("OpenOptimodRefreshPresets")
    static let presetActions = NSToolbarItem.Identifier("OpenOptimodPresetActions")
    static let presetSearch = NSToolbarItem.Identifier("OpenOptimodPresetsSearch")
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
