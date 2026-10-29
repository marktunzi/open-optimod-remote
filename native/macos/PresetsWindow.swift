import AppKit
import Foundation
import UniformTypeIdentifiers

/// The Presets window of one processor: every preset function of the model's
/// own PC Remote (Recall, Save, Save As, Rename, Delete, preset files, Backup
/// and Restore) as a native window. It follows the device session's revision,
/// so a recall on the front panel or by another client shows here too.
final class PresetsWindowController: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate,
    NSSearchFieldDelegate, NSToolbarDelegate, NSMenuDelegate, NSTextFieldDelegate {
    /// The processor this window belongs to; nil until one is connected.
    private(set) var deviceID: String?
    private var api: PresetsAPI
    private let tableView = PresetTableView()
    private let splitViewController = NSSplitViewController()
    private var searchField = NSSearchField()
    private let onAirLabel = NSTextField(labelWithString: "")
    private let statusLabel = NSTextField(labelWithString: "Loading presets…")
    private let recallButton = NSButton(title: "Recall", target: nil, action: nil)
    private let saveButton = NSButton(title: "Save", target: nil, action: nil)
    private let saveAsButton = NSButton(title: "Save As…", target: nil, action: nil)
    private let refreshButton = NSButton()
    private let actionsButton = NSButton()
    private let progress = NSProgressIndicator()
    private var filterButtons: [PresetBrowserFilter: PresetSidebarSourceButton] = [:]
    private var context: PresetBrowserContext?
    private var visible: [DevicePresetPayload] = []
    private var selectedName: String?
    private var filter = PresetBrowserFilter.all
    private var busy = false
    /// What the last operation did; it stays in the status line until the next one starts.
    private var notice: String?
    private var lastRevision: Int?
    private var knownSubtitle: String?
    private var observer: NSObjectProtocol?
    /// The sheet waiting for an answer, for the test hook.
    private var activeAlert: NSAlert?
    private var nameField: NSTextField?
    private var nameProblemLabel: NSTextField?
    var onChanged: (() -> Void)?
    var onClose: ((PresetsWindowController) -> Void)?

    init(deviceID: String?) {
        self.deviceID = deviceID
        api = PresetsAPI(deviceID: deviceID)
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
        window.isReleasedWhenClosed = false
        window.tabbingMode = .disallowed
        window.identifier = NSUserInterfaceItemIdentifier("OpenOptimod.presets")
        super.init(window: window)
        window.delegate = self
        configureWindow()
        updateControls()
        if !window.setFrameUsingName(Self.autosaveName(deviceID)) { window.center() }
        window.setFrameAutosaveName(Self.autosaveName(deviceID))
        observer = NotificationCenter.default.addObserver(forName: .deviceSessionChanged, object: nil, queue: .main) { [weak self] _ in
            self?.sessionChanged()
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    deinit { if let observer { NotificationCenter.default.removeObserver(observer) } }

    static func autosaveName(_ deviceID: String?) -> String { "OpenOptimod.presets.\(deviceID ?? "none")" }

    func show() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        updateSubtitle()
        reload()
    }

    /// Binds a window that was opened before any processor was connected.
    func adopt(deviceID: String) {
        guard self.deviceID == nil, let window else { return }
        self.deviceID = deviceID
        api = PresetsAPI(deviceID: deviceID)
        window.setFrameAutosaveName(Self.autosaveName(deviceID))
    }

    func windowWillClose(_ notification: Notification) { onClose?(self) }

    private var store: DeviceSessionStore { .shared }
    /// The window shows its own processor only, while it is connected.
    private var isOwnDeviceConnected: Bool { store.device(for: deviceID) != nil }

    private func sessionChanged() {
        updateSubtitle()
        guard window?.isVisible == true else { return }
        let revision = store.session(for: deviceID)?.revision
        if revision != lastRevision || isOwnDeviceConnected != (context?.snapshot.connected == true) {
            lastRevision = revision
            if !busy { reload() }
        }
    }

    private func updateSubtitle() {
        guard let window else { return }
        if let device = store.device(for: deviceID) {
            window.subtitle = device.subtitle
            knownSubtitle = device.subtitle
        } else {
            window.subtitle = deviceID == nil ? "Not connected" : [knownSubtitle, "Offline"].compactMap { $0 }.joined(separator: " · ")
        }
    }

    // MARK: Layout

    private func configureWindow() {
        guard let window else { return }
        let toolbar = NSToolbar(identifier: "OpenOptimodPresetsToolbarV2")
        toolbar.delegate = self
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
            button.image = NSImage(systemSymbolName: Self.symbol(for: category), accessibilityDescription: category.rawValue)
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

    private static func symbol(for filter: PresetBrowserFilter) -> String {
        switch filter {
        case .all: return "square.stack.3d.up"
        case .factory: return "building.columns"
        case .user: return "person.crop.circle"
        }
    }

    private func makeContent() -> NSView {
        let content = NSView()
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
        tableView.doubleAction = #selector(recallSelected)
        tableView.onDelete = { [weak self] in self?.deleteSelected() }
        let menu = NSMenu()
        menu.delegate = self
        tableView.menu = menu
        tableView.setAccessibilityLabel("Presets")
        let scroll = NSScrollView()
        scroll.documentView = tableView
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        onAirLabel.font = .systemFont(ofSize: 12, weight: .medium)
        onAirLabel.lineBreakMode = .byTruncatingTail
        statusLabel.textColor = .secondaryLabelColor
        statusLabel.font = .systemFont(ofSize: 12)
        statusLabel.lineBreakMode = .byTruncatingTail
        statusLabel.alignment = .right
        progress.style = .spinning
        progress.controlSize = .small
        progress.isDisplayedWhenStopped = false
        let footer = NSStackView(views: [onAirLabel, NSView(), progress, statusLabel])
        footer.orientation = .horizontal
        footer.alignment = .centerY
        footer.spacing = 8
        footer.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let line = NSBox()
        line.boxType = .separator
        line.translatesAutoresizingMaskIntoConstraints = false

        for view in [scroll, line, footer] { content.addSubview(view) }
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
            scroll.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
            scroll.topAnchor.constraint(equalTo: content.safeAreaLayoutGuide.topAnchor, constant: 8),
            scroll.bottomAnchor.constraint(equalTo: line.topAnchor, constant: -6),
            line.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            line.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            line.bottomAnchor.constraint(equalTo: footer.topAnchor, constant: -7),
            footer.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 18),
            footer.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -18),
            footer.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -8),
            footer.heightAnchor.constraint(equalToConstant: 20),
        ])
        return content
    }

    // MARK: Toolbar

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.recallPreset, .savePreset, .saveAsPreset, .presetActions, .refreshPresets, .flexibleSpace, .presetSearch]
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.recallPreset, .savePreset, .saveAsPreset, .flexibleSpace, .refreshPresets, .presetActions, .presetSearch]
    }

    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier id: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        switch id {
        case .recallPreset:
            return toolbarButton(id: id, button: recallButton, symbol: "play.fill", label: "Recall", action: #selector(recallSelected))
        case .savePreset:
            return toolbarButton(id: id, button: saveButton, symbol: "square.and.arrow.down", label: "Save", action: #selector(saveOnAir))
        case .saveAsPreset:
            return toolbarButton(id: id, button: saveAsButton, symbol: "plus.square.on.square", label: "Save As…", action: #selector(saveOnAirAs))
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

    /// Save and Save As exist only on a model whose firmware stores presets.
    private func syncToolbar() {
        guard let toolbar = window?.toolbar, let context else { return }
        let wanted = context.managesOnDevice
        for id in [NSToolbarItem.Identifier.saveAsPreset, .savePreset] {
            let index = toolbar.items.firstIndex { $0.itemIdentifier == id }
            if wanted, index == nil { toolbar.insertItem(withItemIdentifier: id, at: 1) }
            if !wanted, let index { toolbar.removeItem(at: index) }
        }
    }

    // MARK: Menus

    @objc private func showPresetActions(_ sender: NSButton) {
        let menu = NSMenu()
        menu.autoenablesItems = false
        addItems(to: menu, selection: selectedPreset?.name)
        menu.addItem(.separator())
        let format = context?.fileFormat ?? .forAdapter(nil)
        add(menu, "Apply Preset File…", #selector(applyPresetFile), enabled: idle && context?.canModify == true)
        add(menu, "Save Current Preset as \(format.fileExtension) File…", #selector(savePresetFile), enabled: idle && context?.snapshot.connected == true)
        menu.addItem(.separator())
        add(menu, "Back Up…", #selector(saveBackup), enabled: idle && context?.canRestore == true)
        let files = PresetFileFormat.backsUpFiles(context?.snapshot.adapterID)
        add(menu, files ? "Restore from Backup…" : "Restore System Settings…", #selector(restoreBackup), enabled: idle && context?.canRestore == true)
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: sender.bounds.height + 4), in: sender)
    }

    /// The row's context menu: the actions for the preset under the pointer.
    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        menu.autoenablesItems = false
        let row = tableView.clickedRow
        guard visible.indices.contains(row) else { return }
        if tableView.selectedRow != row { tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false) }
        addItems(to: menu, selection: visible[row].name)
    }

    private func addItems(to menu: NSMenu, selection: String?) {
        add(menu, "Recall…", #selector(recallSelected), enabled: canRecall)
        guard context?.managesOnDevice == true else { return }
        add(menu, "Rename…", #selector(renameSelected), enabled: idle && context?.canRename(selection) == true)
        add(menu, "Delete…", #selector(deleteSelected), enabled: idle && context?.canDelete(selection) == true)
    }

    private func add(_ menu: NSMenu, _ title: String, _ action: Selector, enabled: Bool) {
        let item = menu.addItem(withTitle: title, action: action, keyEquivalent: "")
        item.target = self
        item.isEnabled = enabled
    }

    // MARK: State

    @objc private func selectFilter(_ sender: NSButton) {
        guard let value = sender.identifier?.rawValue, let selected = PresetBrowserFilter(rawValue: value) else { return }
        select(filter: selected)
    }

    private func select(filter selected: PresetBrowserFilter) {
        filter = selected
        for (category, button) in filterButtons { button.state = category == selected ? .on : .off }
        applyFilter()
    }

    @objc private func refreshPresets() {
        guard idle, context?.snapshot.connected == true else { return reload() }
        setBusy(true, message: "Reading presets from the processor…")
        api.json("refresh", body: [:]) { [weak self] result in
            guard let self else { return }
            self.finish(result.map { _ in () }, failure: "Refresh failed", success: nil)
        }
    }

    private func reload() {
        api.load { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error):
                self.context = nil
                self.applyFilter()
                self.showStatus("The service is not available")
                ErrorLogStore.shared.record(source: "Presets", message: error.localizedDescription)
            case let .success(context):
                let own = context.snapshot.connected && (self.deviceID == nil || context.snapshot.deviceID == nil
                    || DeviceRef.identity(deviceID: context.snapshot.deviceID, host: context.snapshot.host) == self.deviceID)
                self.context = own ? context : nil
                self.lastRevision = context.snapshot.revision
                self.syncToolbar()
                self.applyFilter()
                if !self.busy {
                    self.showStatus(own ? self.notice ?? self.countText : "Connect to this Optimod to manage its presets")
                }
            }
        }
    }

    private var countText: String {
        let count = visible.count
        return "\(count) \(count == 1 ? "preset" : "presets")"
    }

    private func applyFilter() {
        let query = searchField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        visible = (context?.snapshot.presets ?? []).filter {
            filter.includes($0) && (query.isEmpty || $0.name.localizedCaseInsensitiveContains(query))
        }
        tableView.reloadData()
        if let selectedName, let row = visible.firstIndex(where: { $0.name == selectedName }) {
            tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
        } else {
            tableView.deselectAll(nil)
        }
        let current = context?.currentName ?? ""
        onAirLabel.stringValue = current.isEmpty ? "" : "On air: \(current)"
        onAirLabel.textColor = current.hasPrefix("modif ") ? .systemOrange : .labelColor
        updateControls()
    }

    private var idle: Bool { !busy && activeAlert == nil }
    private var selectedPreset: DevicePresetPayload? { visible[safe: tableView.selectedRow] }
    private var canRecall: Bool {
        guard idle, context?.ready == true, let preset = selectedPreset else { return false }
        return preset.name != context?.currentName
    }

    private func updateControls() {
        recallButton.isEnabled = canRecall
        let replace = context?.replaceName
        saveButton.isEnabled = idle && replace != nil
        saveButton.toolTip = replace.map { "Save the on-air processing as \($0)" } ?? "Recall a user preset to save changes to it"
        saveAsButton.isEnabled = idle && context?.canSaveAs == true
        refreshButton.isEnabled = !busy
        actionsButton.isEnabled = !busy
        searchField.isEnabled = !busy
    }

    private func setBusy(_ value: Bool, message: String) {
        busy = value
        if value { notice = nil }
        showStatus(message)
        if value { progress.startAnimation(nil) } else { progress.stopAnimation(nil) }
        updateControls()
    }

    private func showStatus(_ message: String) { statusLabel.stringValue = message }

    /// A result that stays in the status line until the next reload.
    private func announce(_ message: String) {
        notice = message
        showStatus(message)
    }

    /// Ends an operation: on success the presets are read again.
    private func finish(_ result: Result<Void, Error>, failure: String, success: String?) {
        onChanged?()
        switch result {
        case let .failure(error):
            setBusy(false, message: "\(failure) · See Error Log")
            ErrorLogStore.shared.record(source: "Presets", message: error.localizedDescription)
            NSSound.beep()
        case .success:
            notice = success
            setBusy(false, message: success ?? "")
        }
        store.refresh()
        reload()
    }

    private var guards: [String: Any] {
        ["confirmed": true, "expected_host": context?.snapshot.host ?? "", "expected_session": context?.snapshot.sessionID ?? ""]
    }

    // MARK: Table

    func numberOfRows(in tableView: NSTableView) -> Int { visible.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard visible.indices.contains(row) else { return nil }
        let preset = visible[row]
        let current = context?.currentName ?? ""
        let onAir = preset.name == current
        let modified = current == "modif \(preset.name)"
        let cell = PresetCellView()
        let symbol = preset.kind == "Factory" ? "building.columns" : "person.crop.circle"
        let icon = NSImageView(image: NSImage(systemSymbolName: symbol, accessibilityDescription: preset.kind) ?? NSImage())
        icon.contentTintColor = .secondaryLabelColor
        icon.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 15, weight: .regular)
        let name = NSTextField(labelWithString: preset.name)
        name.font = .systemFont(ofSize: 13, weight: .medium)
        name.lineBreakMode = .byTruncatingTail
        let kind = NSTextField(labelWithString: preset.kind == "Factory" ? "Factory preset" : "User preset")
        kind.textColor = .secondaryLabelColor
        kind.font = .systemFont(ofSize: 11)
        let labels = NSStackView(views: [name, kind])
        labels.orientation = .vertical
        labels.alignment = .leading
        labels.spacing = 2
        let badge = NSTextField(labelWithString: onAir ? "On Air" : modified ? "On Air · Modified" : "")
        badge.textColor = modified ? .systemOrange : .systemGreen
        badge.font = .systemFont(ofSize: 11, weight: .medium)
        let badgeIcon = NSImageView(image: NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "On Air") ?? NSImage())
        badgeIcon.contentTintColor = modified ? .systemOrange : .systemGreen
        badgeIcon.isHidden = !(onAir || modified)
        cell.badges = [badge, badgeIcon]
        cell.badgeColor = modified ? .systemOrange : .systemGreen
        let state = NSStackView(views: [badgeIcon, badge])
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
            badgeIcon.widthAnchor.constraint(equalToConstant: 14),
            badgeIcon.heightAnchor.constraint(equalToConstant: 14),
        ])
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        if let preset = selectedPreset { selectedName = preset.name } else if tableView.selectedRow < 0 { selectedName = nil }
        updateControls()
    }

    func controlTextDidChange(_ obj: Notification) {
        if let field = obj.object as? NSTextField, field === nameField { validateName(); return }
        applyFilter()
    }

    // MARK: Confirmations

    /// A sheet with a default and a cancel button; `run` follows the default.
    private func confirm(_ title: String, _ detail: String, button: String, destructive: Bool = false,
                         accessory: NSView? = nil, run: @escaping () -> Void) {
        guard let window, activeAlert == nil else { return }
        let alert = NSAlert()
        alert.alertStyle = destructive ? .critical : .warning
        alert.messageText = title
        alert.informativeText = detail
        let action = alert.addButton(withTitle: button)
        if destructive { action.hasDestructiveAction = true }
        alert.addButton(withTitle: "Cancel")
        if let accessory { alert.accessoryView = accessory }
        activeAlert = alert
        updateControls()
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            self.activeAlert = nil
            self.nameField = nil
            self.nameProblemLabel = nil
            self.updateControls()
            if answer == .alertFirstButtonReturn { run() }
        }
        if let nameField { alert.window.makeFirstResponder(nameField); validateName() }
    }

    /// A name field for Save As and Rename, checked as the original client checks it.
    private func nameAccessory(initial: String) -> NSView {
        let field = NSTextField(string: initial)
        field.placeholderString = "Preset name"
        field.delegate = self
        field.setAccessibilityLabel("Preset name")
        field.widthAnchor.constraint(equalToConstant: 260).isActive = true
        let problem = NSTextField(labelWithString: "")
        problem.textColor = .secondaryLabelColor
        problem.font = .systemFont(ofSize: 11)
        let stack = NSStackView(views: [field, problem])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 4
        stack.frame = NSRect(x: 0, y: 0, width: 260, height: 44)
        nameField = field
        nameProblemLabel = problem
        return stack
    }

    private func validateName() {
        guard let field = nameField, let context else { return }
        let problem = context.nameProblem(field.stringValue)
        nameProblemLabel?.stringValue = problem ?? "Up to \(context.fileFormat.maxNameLength) characters."
        activeAlert?.buttons.first?.isEnabled = problem == nil
    }

    // MARK: Actions

    @objc private func recallSelected() {
        guard canRecall, let context, let preset = selectedPreset, let session = context.snapshot.sessionID else { return }
        let current = context.currentName
        // Only processors with an HD path say so.
        let hd = ["pc-remote-5700i-", "pc-remote-8700hd-", "pc-remote-8700i-"].contains { context.snapshot.adapterID?.hasPrefix($0) == true }
        confirm("Recall \(preset.name)?",
                "This replaces \(current.isEmpty ? "the on-air preset" : current) and changes the on-air sound. \(hd ? "FM and HD use the processing settings stored in this preset" : "The processing settings stored in this preset go on air"); output routing is not changed.",
                button: "Recall") { [weak self] in
            guard let self else { return }
            let request = PresetRecallRequest(name: preset.name, expectedName: current, confirmed: true,
                                              expectedHost: context.snapshot.host, expectedSession: session)
            self.setBusy(true, message: "Recalling \(preset.name)…")
            self.api.recall(request) { result in
                self.selectedName = preset.name
                self.finish(result, failure: "Recall failed", success: "\(preset.name) is on air")
            }
        }
    }

    @objc private func saveOnAir() {
        guard idle, let context, let name = context.replaceName else { return }
        confirm("Save \(name)?", "The user preset \(name) on the processor is replaced with the on-air processing.",
                button: "Save") { [weak self] in
            self?.manage(["action": "save", "name": name], expected: context.currentName,
                         progress: "Saving \(name)…", failure: "Save failed", success: "Saved \(name)")
        }
    }

    @objc private func saveOnAirAs() {
        guard idle, let context, context.canSaveAs else { return }
        let accessory = nameAccessory(initial: "")
        confirm("Save the on-air preset as…", "The on-air processing is stored on the processor as a new user preset.",
                button: "Save", accessory: accessory) { [weak self, field = nameField] in
            guard let self, let name = field?.stringValue, self.context?.nameProblem(name) == nil else { return }
            self.selectedName = name
            self.manage(["action": "save", "name": name], expected: context.currentName,
                        progress: "Saving \(name)…", failure: "Save As failed", success: "Saved \(name)")
        }
    }

    @objc private func renameSelected() {
        guard idle, let context, let preset = selectedPreset, context.canRename(preset.name) else { return }
        let accessory = nameAccessory(initial: preset.name)
        confirm("Rename \(preset.name)?", "The app saves the preset under the new name, then deletes the old one, as the original PC Remote does.",
                button: "Rename", accessory: accessory) { [weak self, field = nameField] in
            guard let self, let name = field?.stringValue, self.context?.nameProblem(name) == nil else { return }
            self.selectedName = name
            self.manage(["action": "rename", "name": preset.name, "new_name": name], expected: context.currentName,
                        progress: "Renaming \(preset.name)…", failure: "Rename failed", success: "Renamed to \(name)")
        }
    }

    @objc private func deleteSelected() {
        guard idle, let context, let preset = selectedPreset, context.canDelete(preset.name) else { return }
        confirm("Delete \(preset.name)?", "The user preset is deleted from the processor. This cannot be undone.",
                button: "Delete", destructive: true) { [weak self] in
            self?.selectedName = nil
            self?.manage(["action": "delete", "name": preset.name], expected: context.currentName,
                         progress: "Deleting \(preset.name)…", failure: "Delete failed", success: "Deleted \(preset.name)")
        }
    }

    private func manage(_ action: [String: Any], expected: String, progress: String, failure: String, success: String) {
        var body = guards
        body.merge(action) { $1 }
        body["expected_name"] = expected
        setBusy(true, message: progress)
        api.json("presets/manage", body: body) { [weak self] result in
            self?.finish(result.map { _ in () }, failure: failure, success: success)
        }
    }

    @objc private func savePresetFile() {
        guard idle, context?.snapshot.connected == true else { return }
        let format = context?.fileFormat ?? .forAdapter(nil)
        setBusy(true, message: "Reading the on-air preset…")
        api.currentFile { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error):
                self.finish(.failure(error), failure: "Saving the preset file failed", success: nil)
            case let .success(file):
                self.setBusy(false, message: "")
                let name = PresetBackup.safeFilename(file.name) + "." + format.fileExtension
                self.chooseSaveLocation(title: "Save the On-Air \(format.model) Preset", name: name,
                                        type: UTType(filenameExtension: format.fileExtension) ?? .data) { url in
                    try file.document.write(to: url, atomically: true, encoding: .utf8)
                    self.announce("Saved \(url.lastPathComponent)")
                }
            }
        }
    }

    @objc private func applyPresetFile() {
        guard idle, let context, context.canModify, let session = context.snapshot.sessionID else { return }
        let format = context.fileFormat
        chooseFiles(title: "Choose an \(format.model) Preset File", multiple: false, folders: false,
                    types: [UTType(filenameExtension: format.fileExtension) ?? .data, UTType(filenameExtension: "orb") ?? .data, .plainText]) { [weak self] urls in
            guard let self, let url = urls.first else { return }
            let data = try Data(contentsOf: url, options: [.mappedIfSafe])
            guard data.count <= 65_535, let document = String(data: data, encoding: .utf8) else {
                throw SystemSettingsAPIError.service("This is not a supported \(format.model) preset file.")
            }
            let current = context.currentName
            self.confirm("Apply \(url.lastPathComponent)?",
                         "Every processing value in the file is written to \(current.isEmpty ? "the on-air preset" : current) and read back. This changes the on-air sound.",
                         button: "Apply") {
                let request = PresetApplyRequest(document: document, expectedName: current, confirmed: true,
                                                 expectedHost: context.snapshot.host, expectedSession: session)
                self.setBusy(true, message: "Applying and verifying \(url.lastPathComponent)…")
                self.api.apply(request) { result in
                    self.finish(result, failure: "Applying the preset file failed", success: "Applied \(url.lastPathComponent)")
                }
            }
        }
    }

    /// The original PC Remote's plain-text backup (setup file and every user
    /// preset file) where the model has one, else this app's JSON backup.
    @objc private func saveBackup() {
        guard idle, let context, context.canRestore else { return }
        let date = ISO8601DateFormatter.string(from: Date(), timeZone: TimeZone(identifier: "UTC")!, formatOptions: [.withFullDate])
        if PresetFileFormat.backsUpFiles(context.snapshot.adapterID) {
            chooseBackupFormat { [weak self] password in
                self?.saveBackupFiles(context: context, date: date, password: password)
            }
            return
        }
        setBusy(true, message: "Reading the system settings and the on-air preset…")
        api.json("backup") { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error): self.finish(.failure(error), failure: "Backup failed", success: nil)
            case let .success(json):
                self.setBusy(false, message: "")
                let model = PresetBackup.safeFilename(json["model"] as? String ?? "Optimod")
                self.chooseSaveLocation(title: "Save Backup", name: "\(model) backup \(date).json", type: .json) { url in
                    try JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys]).write(to: url, options: .atomic)
                    self.announce("Backup saved: the system settings, the on-air preset and the preset names")
                }
            }
        }
    }

    /// The client's "Choose backup file format": plain text, encrypted with
    /// the session's password, or with a password entered here. Calls back
    /// with nil for plain text, "" for the session password, else the password.
    private func chooseBackupFormat(then: @escaping (String?) -> Void) {
        guard let window else { return }
        let alert = NSAlert()
        alert.messageText = "Choose backup file format"
        let format = NSPopUpButton()
        format.addItems(withTitles: ["Save backup files in plain text.",
                                     "Encrypt backup files with current session password.",
                                     "Encrypt backup files with password entered below."])
        let password = NSSecureTextField()
        password.placeholderString = "Enter Password"
        let confirm = NSSecureTextField()
        confirm.placeholderString = "Confirm Password"
        format.target = self
        format.action = #selector(backupFormatChanged(_:))
        password.isEnabled = false
        confirm.isEnabled = false
        let stack = NSStackView(views: [format, password, confirm])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.frame = NSRect(x: 0, y: 0, width: 340, height: 90)
        for view in [format, password, confirm] { view.widthAnchor.constraint(equalToConstant: 340).isActive = true }
        alert.accessoryView = stack
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        activeAlert = alert
        alert.beginSheetModal(for: window) { [weak self] response in
            self?.activeAlert = nil
            guard response == .alertFirstButtonReturn else { return }
            switch format.indexOfSelectedItem {
            case 0: then(nil)
            case 1: then("")
            default:
                guard !password.stringValue.isEmpty, password.stringValue == confirm.stringValue else {
                    self?.showStatus("The passwords are empty or do not match; no backup was saved")
                    NSSound.beep()
                    return
                }
                then(password.stringValue)
            }
        }
    }

    /// The password boxes count only for "password entered below".
    @objc private func backupFormatChanged(_ sender: NSPopUpButton) {
        let entered = sender.indexOfSelectedItem == 2
        sender.superview?.subviews.compactMap { $0 as? NSSecureTextField }.forEach { $0.isEnabled = entered }
    }

    private func saveBackupFiles(context: PresetBrowserContext, date: String, password: String?) {
        setBusy(true, message: "Reading the setup file and every user preset file…")
        api.json("backup/files") { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error): self.finish(.failure(error), failure: "Backup failed", success: nil)
            case let .success(json):
                let plain = (json["files"] as? [[String: Any]] ?? []).compactMap { file -> (name: String, document: String)? in
                    guard let name = file["name"] as? String, let document = file["document"] as? String else { return nil }
                    return (name, document)
                }
                guard let password else {
                    self.setBusy(false, message: "")
                    return self.writeBackup(context: context, date: date, files: plain.map { ($0.name, Data($0.document.utf8)) })
                }
                let body: [String: Any] = ["files": plain.map { ["name": $0.name, "document": $0.document] },
                                           "password": password.isEmpty ? NSNull() : password]
                self.api.json("backup/encrypt", body: body) { result in
                    switch result {
                    case let .failure(error): self.finish(.failure(error), failure: "Backup failed", success: nil)
                    case let .success(json):
                        self.setBusy(false, message: "")
                        let files = (json["files"] as? [[String: Any]] ?? []).compactMap { file -> (name: String, data: Data)? in
                            guard let name = file["name"] as? String, let hex = file["data"] as? String,
                                  let data = Data(hexadecimal: hex) else { return nil }
                            return (name, data)
                        }
                        self.writeBackup(context: context, date: date, files: files)
                    }
                }
            }
        }
    }

    private func writeBackup(context: PresetBrowserContext, date: String, files: [(name: String, data: Data)]) {
        let folder = "\(PresetBackup.safeFilename(context.snapshot.host.isEmpty ? "Optimod" : context.snapshot.host)) backup \(date)"
        chooseSaveLocation(title: "Save Backup", name: "\(folder).zip", type: .zip) { [weak self] url in
            try PresetBackup.zip(folder: folder, files: files).write(to: url, options: .atomic)
            self?.announce("Backup saved: \(files.map(\.name).joined(separator: ", "))")
        }
    }

    @objc private func restoreBackup() {
        guard idle, let context, context.canRestore else { return }
        let adapterID = context.snapshot.adapterID
        if PresetFileFormat.backsUpFiles(adapterID) {
            let others = ["orb57setup"] + PresetFileFormat.restoresAsFiles(adapterID).map { String($0.dropFirst()) }
            let types = ([context.fileFormat.fileExtension] + others).map { UTType(filenameExtension: $0) ?? .data }
            chooseFiles(title: "Choose the Backup Folder or Files", multiple: true, folders: true, types: types) { [weak self] urls in
                guard let self else { return }
                let raw = try Self.readBackupFiles(urls, adapterID: adapterID)
                self.decryptBackup(raw, password: nil) { files in
                    do { try self.restoreFiles(files, adapterID: adapterID) }
                    catch { self.finish(.failure(error), failure: "Restore failed", success: nil) }
                }
            }
            return
        }
        restoreJSONBackup(adapterID: adapterID)
    }

    /// Encrypted files of the client's backup, decrypted by the service with
    /// the session's password first, then with one the user enters.
    private func decryptBackup(_ raw: [(name: String, data: Data, modified: UInt64?)], password: String?,
                               then: @escaping ([PresetBackupFile]) -> Void) {
        let plain = raw.compactMap { file -> PresetBackupFile? in
            guard file.data.isEmpty || file.data.starts(with: Data("OptimodVersion=<".utf8)),
                  let document = String(data: file.data, encoding: .utf8) else { return nil }
            return PresetBackupFile(name: file.name, document: document, modified: file.modified)
        }
        guard plain.count < raw.count else { return then(plain) }
        setBusy(true, message: "Decrypting the backup files…")
        let body: [String: Any] = ["files": raw.map { ["name": $0.name, "data": $0.data.hexadecimal] },
                                   "password": password ?? NSNull()]
        api.json("backup/decrypt", body: body) { [weak self] result in
            guard let self else { return }
            self.setBusy(false, message: "")
            switch result {
            case let .success(json):
                let files = (json["files"] as? [[String: Any]] ?? []).enumerated().compactMap { index, file -> PresetBackupFile? in
                    guard let name = file["name"] as? String, let document = file["document"] as? String else { return nil }
                    return PresetBackupFile(name: name, document: document, modified: raw.indices.contains(index) ? raw[index].modified : nil)
                }
                then(files)
            case let .failure(error) where error.localizedDescription.contains("encrypted with another password"):
                self.askBackupPassword(retry: password != nil) { entered in
                    self.decryptBackup(raw, password: entered, then: then)
                }
            case let .failure(error):
                self.finish(.failure(error), failure: "Restore failed", success: nil)
            }
        }
    }

    private func askBackupPassword(retry: Bool, then: @escaping (String) -> Void) {
        guard let window else { return }
        let alert = NSAlert()
        alert.messageText = retry ? "That password does not open this backup" : "This backup is encrypted"
        alert.informativeText = "Enter the password it was saved with."
        let field = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 24))
        alert.accessoryView = field
        alert.addButton(withTitle: "Open")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        activeAlert = alert
        alert.beginSheetModal(for: window) { [weak self] response in
            self?.activeAlert = nil
            if response == .alertFirstButtonReturn, !field.stringValue.isEmpty { then(field.stringValue) }
        }
    }

    private func restoreFiles(_ files: [PresetBackupFile], adapterID: String?) throws {
                let sorted = try PresetBackup.sort(files, adapterID: adapterID)
                guard sorted.setup != nil || !sorted.presets.isEmpty else {
                    throw SystemSettingsAPIError.service("There is no setup or preset file in this selection.")
                }
                self.plan(setup: sorted.setup?.document) { changes, skipped in
                    var lines: [String] = []
                    let others = PresetFileFormat.restoresAsFiles(adapterID)
                    let isOther = { (file: PresetBackupFile) in others.contains { file.name.lowercased().hasSuffix($0) } }
                    let userFiles = sorted.presets.filter { !isOther($0) }, otherFiles = sorted.presets.filter(isOther)
                    if !userFiles.isEmpty {
                        lines.append("User preset files: \(userFiles.map(\.name).joined(separator: ", ")). A preset with the same name is replaced; the on-air sound does not change.")
                    }
                    if !otherFiles.isEmpty {
                        lines.append("\(otherFiles.map(\.name).joined(separator: ", ")): these replace the processor's own setup, automation or network file, as the original client's Restore does. A network file can change its IP address.")
                    }
                    if sorted.setup != nil {
                        lines.append(changes.isEmpty ? "The setup file matches the processor's system settings; nothing to restore there."
                            : "System settings (\(changes.count)): \(changes.joined(separator: ", ")). Each change is written once and read back.")
                    }
                    if !skipped.isEmpty { lines.append("Left unchanged: \(skipped.joined(separator: "; ")).") }
                    guard !sorted.presets.isEmpty || !changes.isEmpty else {
                        self.showStatus("Nothing to restore: the setup file matches the processor")
                        return
                    }
                    self.confirm("Restore from this backup?", lines.joined(separator: "\n\n"), button: "Restore") {
                        self.restore(presets: sorted.presets, setup: changes.isEmpty ? nil : sorted.setup?.document)
                    }
                }
    }

    private func restoreJSONBackup(adapterID: String?) {
        chooseFiles(title: "Choose an Open Optimod Remote Backup", multiple: false, folders: false, types: [.json]) { [weak self] urls in
            guard let self, let url = urls.first else { return }
            let setup = try PresetBackup.systemDocument(of: Data(contentsOf: url), adapterID: adapterID)
            self.plan(setup: setup) { changes, skipped in
                guard !changes.isEmpty else { return self.showStatus("Nothing to restore: the backup matches the processor") }
                var detail = "\(changes.count) system \(changes.count == 1 ? "setting" : "settings"): \(changes.joined(separator: ", ")). Each change is written once and read back. Network settings and the clock are never changed."
                if !skipped.isEmpty { detail += "\n\nLeft unchanged: \(skipped.joined(separator: "; "))." }
                self.confirm("Restore system settings from \(url.lastPathComponent)?", detail, button: "Restore") {
                    self.restore(presets: [], setup: setup)
                }
            }
        }
    }

    /// The files of a selection; a chosen folder contributes its setup and preset files.
    private static func readBackupFiles(_ urls: [URL], adapterID: String?) throws -> [(name: String, data: Data, modified: UInt64?)] {
        let presetExtension = PresetFileFormat.forAdapter(adapterID).fileExtension
        var chosen: [URL] = []
        for url in urls {
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                let contents = try FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: [.contentModificationDateKey])
                chosen += contents.filter { $0.lastPathComponent.lowercased() == "setup.orb57setup" || $0.pathExtension.lowercased() == presetExtension }
                    .sorted { $0.lastPathComponent < $1.lastPathComponent }
            } else {
                chosen.append(url)
            }
        }
        return try chosen.map { url in
            let data = try Data(contentsOf: url)
            guard data.count <= 65_535 else {
                throw SystemSettingsAPIError.service("\(url.lastPathComponent) is not a backup file of this processor.")
            }
            let date = try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
            return (url.lastPathComponent, data, date.map { UInt64($0.timeIntervalSince1970 * 1000) })
        }
    }

    /// The setup changes a restore would write, as the service plans them.
    private func plan(setup: String?, then: @escaping (_ changes: [String], _ skipped: [String]) -> Void) {
        guard let setup else { return then([], []) }
        setBusy(true, message: "Comparing the setup with the processor…")
        api.json("backup/setup-plan", body: ["document": setup]) { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error): self.finish(.failure(error), failure: "Restore failed", success: nil)
            case let .success(json):
                self.setBusy(false, message: "")
                let changes = (json["changes"] as? [[String: Any]] ?? []).compactMap { $0["name"] as? String }
                let skipped = (json["skipped"] as? [[String: Any]] ?? []).compactMap { item -> String? in
                    guard let name = item["name"] as? String else { return nil }
                    return "\(name): \(item["reason"] as? String ?? "")"
                }
                then(changes, skipped)
            }
        }
    }

    private func restore(presets: [PresetBackupFile], setup: String?) {
        setBusy(true, message: presets.isEmpty ? "Restoring system settings…" : "Restoring \(presets.count) \(presets.count == 1 ? "file" : "files")…")
        let done: (Result<Void, Error>) -> Void = { [weak self] result in
            let what = [presets.isEmpty ? nil : "\(presets.count) \(presets.count == 1 ? "preset" : "presets")", setup == nil ? nil : "the system settings"]
                .compactMap { $0 }.joined(separator: " and ")
            self?.finish(result, failure: "Restore failed", success: "Restored \(what)")
        }
        let restoreSetup = { [weak self] in
            guard let self, let setup else { return done(.success(())) }
            var body = self.guards
            body["document"] = setup
            self.showStatus("Restoring system settings…")
            self.api.json("backup/restore-setup", body: body) { done($0.map { _ in () }) }
        }
        guard !presets.isEmpty else { return restoreSetup() }
        var body = guards
        body["files"] = presets.map { file -> [String: Any] in
            var entry: [String: Any] = ["name": file.name, "document": file.document]
            if let modified = file.modified { entry["modified"] = modified }
            return entry
        }
        api.json("backup/restore-presets", body: body) { result in
            if case let .failure(error) = result { return done(.failure(error)) }
            restoreSetup()
        }
    }

    // MARK: Panels

    private func chooseSaveLocation(title: String, name: String, type: UTType, write: @escaping (URL) throws -> Void) {
        let save = { [weak self] (url: URL) in
            do { try write(url) } catch {
                self?.finish(.failure(error), failure: "Saving failed", success: nil)
            }
        }
        if let folder = FilePanelTestHook.nextSaveFolder {
            FilePanelTestHook.nextSaveFolder = nil
            return save(folder.appendingPathComponent(name))
        }
        guard let window else { return }
        let panel = NSSavePanel()
        panel.title = title
        panel.nameFieldStringValue = name
        panel.allowedContentTypes = [type]
        panel.canCreateDirectories = true
        panel.beginSheetModal(for: window) { response in
            if response == .OK, let url = panel.url { save(url) }
        }
    }

    private func chooseFiles(title: String, multiple: Bool, folders: Bool, types: [UTType], use: @escaping ([URL]) throws -> Void) {
        let run = { [weak self] (urls: [URL]) in
            do { try use(urls) } catch {
                self?.finish(.failure(error), failure: "The file could not be used", success: nil)
            }
        }
        if let files = FilePanelTestHook.nextOpen {
            FilePanelTestHook.nextOpen = nil
            return run(files)
        }
        guard let window else { return }
        let panel = NSOpenPanel()
        panel.title = title
        panel.allowedContentTypes = types
        panel.allowsMultipleSelection = multiple
        panel.canChooseDirectories = folders
        panel.canChooseFiles = true
        panel.beginSheetModal(for: window) { response in
            if response == .OK { run(panel.urls) }
        }
    }

    // MARK: Test hook

    /// `presets WORDS…` (OPEN_OPTIMOD_WINDOW_CONTROL) acts as the window's controls do.
    func perform(_ words: [String]) {
        guard let verb = words.first else { return }
        let rest = words.dropFirst().joined(separator: " ")
        switch verb {
        case "filter": select(filter: PresetBrowserFilter.allCases.first { $0.rawValue.lowercased().hasPrefix(rest.lowercased()) } ?? .all)
        case "search":
            searchField.stringValue = rest
            applyFilter()
        case "select":
            if let row = visible.firstIndex(where: { $0.name == rest }) {
                tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
                tableView.scrollRowToVisible(row)
            }
        case "recall": recallSelected()
        case "save": saveOnAir()
        case "saveas": saveOnAirAs()
        case "rename": renameSelected()
        case "delete": deleteSelected()
        case "apply": applyPresetFile()
        case "savefile": savePresetFile()
        case "backup": saveBackup()
        case "restore": restoreBackup()
        case "refresh": refreshPresets()
        case "name":
            nameField?.stringValue = rest
            validateName()
        case "accessory":
            // Test hook: `accessory N VALUE` sets the Nth control of the open
            // sheet's extra view: a text field's text or a pop-up's item number.
            let parts = rest.split(separator: " ", maxSplits: 1).map(String.init)
            guard let index = parts.first.flatMap(Int.init), let view = activeAlert?.accessoryView else { return }
            let controls = ([view] + view.subviews).filter { $0 is NSTextField || $0 is NSPopUpButton }
            guard controls.indices.contains(index) else { return }
            let value = parts.count > 1 ? parts[1] : ""
            if let popup = controls[index] as? NSPopUpButton {
                popup.selectItem(at: Int(value) ?? 0)
                if let action = popup.action { NSApp.sendAction(action, to: popup.target, from: popup) }
            }
            else if let field = controls[index] as? NSTextField { field.stringValue = value }
        case "confirm", "cancel":
            guard let alert = activeAlert, let window else { return }
            if verb == "confirm", alert.buttons.first?.isEnabled == false { return }
            window.endSheet(alert.window, returnCode: verb == "confirm" ? .alertFirstButtonReturn : .alertSecondButtonReturn)
        default: break
        }
    }

    /// What the window shows, for the test hook.
    var testState: [String: Any] {
        [
            "kind": "native-presets",
            "device": deviceID ?? "",
            "visible": window?.isVisible ?? false,
            "window_number": window?.windowNumber ?? 0,
            "subtitle": window?.subtitle ?? "",
            "status": statusLabel.stringValue,
            "on_air": onAirLabel.stringValue,
            "selected": selectedPreset?.name ?? "",
            "rows": visible.count,
            "alert": activeAlert.map { "\($0.messageText) | \($0.informativeText)" } ?? "",
            "name_problem": nameProblemLabel?.stringValue ?? "",
            "busy": busy,
            "toolbar": window?.toolbar?.items.map(\.label) ?? [],
            "enabled": ["recall": recallButton.isEnabled, "save": saveButton.isEnabled, "save_as": saveAsButton.isEnabled],
        ]
    }
}

/// A preset row; its On Air badge turns white on a selected row, like the labels.
private final class PresetCellView: NSTableCellView {
    var badges: [NSView] = []
    var badgeColor = NSColor.systemGreen

    override var backgroundStyle: NSView.BackgroundStyle {
        didSet {
            let color = backgroundStyle == .emphasized ? NSColor.alternateSelectedControlTextColor : badgeColor
            for view in badges {
                (view as? NSTextField)?.textColor = color
                (view as? NSImageView)?.contentTintColor = color
            }
        }
    }
}

/// The preset table; Delete removes the selected user preset after asking.
private final class PresetTableView: NSTableView {
    var onDelete: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 51 || event.keyCode == 117 { onDelete?() } else { super.keyDown(with: event) }
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

enum PresetPresentation {
    static let defaultSize = NSSize(width: 900, height: 560)
    static let minimumSize = NSSize(width: 760, height: 460)
    static let rowHeight: CGFloat = 40
    static let sidebarWidth: CGFloat = 188
    static let minimumSidebarWidth: CGFloat = 168
    static let maximumSidebarWidth: CGFloat = 240
}

private extension NSToolbarItem.Identifier {
    static let recallPreset = NSToolbarItem.Identifier("OpenOptimodRecallPreset")
    static let savePreset = NSToolbarItem.Identifier("OpenOptimodSavePreset")
    static let saveAsPreset = NSToolbarItem.Identifier("OpenOptimodSaveAsPreset")
    static let refreshPresets = NSToolbarItem.Identifier("OpenOptimodRefreshPresets")
    static let presetActions = NSToolbarItem.Identifier("OpenOptimodPresetActions")
    static let presetSearch = NSToolbarItem.Identifier("OpenOptimodPresetsSearch")
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

extension Data {
    /// Lowercase hexadecimal, as the service's backup file data.
    var hexadecimal: String { map { String(format: "%02x", $0) }.joined() }

    init?(hexadecimal text: String) {
        guard text.count % 2 == 0 else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(text.count / 2)
        var index = text.startIndex
        while index < text.endIndex {
            let next = text.index(index, offsetBy: 2)
            guard let byte = UInt8(text[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self.init(bytes)
    }
}
