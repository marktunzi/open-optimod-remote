import AppKit
import Foundation

final class ConnectionsWindowController: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate, NSToolbarDelegate {
    private let api = ConnectionsAPI()
    private let tableView = NSTableView()
    private let splitViewController = NSSplitViewController()
    private var splitView: NSSplitView { splitViewController.splitView }
    private var searchField = NSSearchField()
    private let subtitleLabel = NSTextField(labelWithString: "Saved OPTIMOD processors")
    private let toolbarTitle = NSTextField(labelWithString: "All Connections")
    private let allButton = SidebarSourceButton()
    private let networkButton = SidebarSourceButton()
    private let addButton = NSButton()
    private let headerAddButton = NSButton()
    private let moreButton = NSButton()
    private let viewControl = NSSegmentedControl()
    private let removeButton = NSButton()
    private let connectButton = NSButton()
    private let editButton = NSButton()
    private let detailName = NSTextField(labelWithString: "Select a connection")
    private let detailAddress = NSTextField(labelWithString: "")
    private let detailCode = NSTextField(labelWithString: "")
    private let progress = NSProgressIndicator()
    private var context = ConnectionsContext(devices: [], connected: false, activeID: nil)
    private var visibleDevices: [SavedConnection] = []
    private var selectedID: String?
    private var busy = false
    private var showingNetwork = false
    private var discovered: [DiscoveredOptimod] = []
    private var discovery: OptimodNetworkDiscovery?
    private var connectionEditor: ConnectionEditorWindowController?
    private var statusTimer: Timer?
    private var statusRefreshInFlight = false
    var onConnected: (() -> Void)?

    init() {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: ConnectionsPresentation.defaultSize),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "All Connections"
        window.contentMinSize = ConnectionsPresentation.minimumSize
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.toolbarStyle = .unified
        window.titleVisibility = .hidden
        super.init(window: window)
        window.delegate = self
        configureWindow()
        window.setFrameAutosaveName("OpenOptimodRemoteConnectionsWindowV3")
        window.center()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func show() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        reload()
        startStatusTimer()
    }

    func refreshStatusNow() {
        refreshStatus()
    }

    func windowWillClose(_ notification: Notification) {
        statusTimer?.invalidate()
        statusTimer = nil
        discovery?.cancel()
    }

    private func startStatusTimer() {
        statusTimer?.invalidate()
        statusTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.refreshStatus()
        }
    }

    private func refreshStatus() {
        guard !busy, !statusRefreshInFlight, window?.isVisible == true else { return }
        statusRefreshInFlight = true
        api.load { [weak self] result in
            guard let self else { return }
            self.statusRefreshInFlight = false
            guard case let .success(updated) = result else { return }
            guard updated.connected != self.context.connected
                    || updated.activeID != self.context.activeID
                    || updated.devices != self.context.devices
            else { return }
            self.context = updated
            if let selectedID = self.selectedID,
               !updated.devices.contains(where: { $0.id == selectedID }) {
                self.selectedID = updated.devices.first?.id
            }
            self.filterAndReload()
            let subtitle = "\(updated.devices.count) saved processor\(updated.devices.count == 1 ? "" : "s")"
            self.subtitleLabel.stringValue = subtitle
        }
    }

    private func configureWindow() {
        guard let window else { return }
        let sidebarController = NSViewController()
        sidebarController.view = makeSidebar()
        let contentController = NSViewController()
        contentController.view = makeContent()

        let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebarController)
        sidebarItem.minimumThickness = ConnectionsPresentation.minimumSidebarWidth
        sidebarItem.maximumThickness = ConnectionsPresentation.maximumSidebarWidth
        sidebarItem.automaticMaximumThickness = 240
        sidebarItem.preferredThicknessFraction = ConnectionsPresentation.sidebarWidth / ConnectionsPresentation.defaultSize.width
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
        splitViewController.splitView.setPosition(ConnectionsPresentation.sidebarWidth, ofDividerAt: 0)

        let toolbar = NSToolbar(identifier: "OpenOptimodConnectionsToolbarV2")
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        window.toolbar = toolbar
    }

    private func makeSidebar() -> NSView {
        let sidebar = NSVisualEffectView()
        sidebar.material = .sidebar
        sidebar.blendingMode = .behindWindow
        sidebar.state = .followsWindowActiveState

        allButton.title = "All Connections"
        allButton.image = NSImage(systemSymbolName: "rectangle.stack.badge.person.crop", accessibilityDescription: "All Connections")
            ?? NSImage(systemSymbolName: "server.rack", accessibilityDescription: "All Connections")
        allButton.imagePosition = .imageLeading
        allButton.alignment = .left
        allButton.setButtonType(.toggle)
        allButton.isBordered = false
        allButton.state = .on
        allButton.target = self
        allButton.action = #selector(showAllConnections)
        allButton.font = .systemFont(ofSize: 13)
        allButton.contentTintColor = .labelColor

        networkButton.title = "Network"
        networkButton.image = NSImage(systemSymbolName: "network", accessibilityDescription: "Network")
        networkButton.imagePosition = .imageLeading
        networkButton.alignment = .left
        networkButton.setButtonType(.toggle)
        networkButton.isBordered = false
        networkButton.target = self
        networkButton.action = #selector(showNetworkConnections)
        networkButton.font = .systemFont(ofSize: 13)
        networkButton.contentTintColor = .labelColor

        let sourceStack = NSStackView(views: [allButton, networkButton])
        sourceStack.orientation = .vertical
        sourceStack.spacing = 6
        sourceStack.alignment = .leading
        sourceStack.translatesAutoresizingMaskIntoConstraints = false
        for button in [allButton, networkButton] {
            button.widthAnchor.constraint(equalTo: sourceStack.widthAnchor).isActive = true
            button.heightAnchor.constraint(equalToConstant: 32).isActive = true
        }

        addButton.image = NSImage(systemSymbolName: "plus", accessibilityDescription: "Add Connection")
        addButton.title = ""
        addButton.bezelStyle = .inline
        addButton.target = self
        addButton.action = #selector(addConnection)
        addButton.toolTip = "Add Connection"

        removeButton.image = NSImage(systemSymbolName: "minus", accessibilityDescription: "Remove Connection")
        removeButton.title = ""
        removeButton.bezelStyle = .inline
        removeButton.target = self
        removeButton.action = #selector(removeConnection)
        removeButton.toolTip = "Remove Connection"

        let sourceActions = NSStackView(views: [addButton, NSBox(), removeButton])
        sourceActions.orientation = .horizontal
        sourceActions.spacing = 8
        sourceActions.translatesAutoresizingMaskIntoConstraints = false
        if let spacer = sourceActions.views[1] as? NSBox {
            spacer.boxType = .custom
            spacer.fillColor = .clear
            spacer.borderWidth = 0
        }

        sidebar.addSubview(sourceStack)
        sidebar.addSubview(sourceActions)
        NSLayoutConstraint.activate([
            sourceStack.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 14),
            sourceStack.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -14),
            sourceStack.topAnchor.constraint(equalTo: sidebar.topAnchor, constant: 54),
            sourceActions.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
            sourceActions.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -12),
            sourceActions.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor, constant: -10),
            sourceActions.heightAnchor.constraint(equalToConstant: 28),
        ])
        return sidebar
    }

    private func makeContent() -> NSView {
        let content = NSView()

        let connectionColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("connection"))
        connectionColumn.resizingMask = .autoresizingMask
        tableView.addTableColumn(connectionColumn)
        tableView.headerView = nil
        tableView.rowHeight = ConnectionsPresentation.rowHeight
        tableView.intercellSpacing = NSSize(width: 0, height: 2)
        tableView.style = .fullWidth
        tableView.usesAlternatingRowBackgroundColors = false
        tableView.allowsEmptySelection = true
        tableView.allowsMultipleSelection = false
        tableView.delegate = self
        tableView.dataSource = self
        tableView.target = self
        tableView.doubleAction = #selector(connectSelected)
        let scroll = NSScrollView()
        scroll.documentView = tableView
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        connectButton.title = "Connect"
        connectButton.bezelStyle = .rounded
        connectButton.keyEquivalent = "\r"
        connectButton.target = self
        connectButton.action = #selector(connectSelected)
        editButton.title = "Edit…"
        editButton.bezelStyle = .rounded
        editButton.target = self
        editButton.action = #selector(editConnection)
        progress.style = .spinning
        progress.controlSize = .small
        progress.isDisplayedWhenStopped = false
        progress.translatesAutoresizingMaskIntoConstraints = false

        content.addSubview(scroll)
        content.addSubview(progress)
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
        [.toggleSidebar, .sidebarTrackingSeparator, .browserTitle, .viewMode, .actions, .addConnection, .flexibleSpace, .search]
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.toggleSidebar, .sidebarTrackingSeparator, .browserTitle, .flexibleSpace, .viewMode, .actions, .addConnection, .search]
    }

    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier id: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        switch id {
        case .toggleSidebar:
            let item = NSToolbarItem(itemIdentifier: id)
            item.label = "Sidebar"
            item.paletteLabel = "Show or Hide Sidebar"
            item.image = NSImage(systemSymbolName: "sidebar.left", accessibilityDescription: "Show or Hide Sidebar")
            item.target = self
            item.action = #selector(toggleSidebar)
            return item
        case .sidebarTrackingSeparator:
            return NSTrackingSeparatorToolbarItem(identifier: id, splitView: splitView, dividerIndex: 0)
        case .browserTitle:
            toolbarTitle.font = .systemFont(ofSize: 15, weight: .semibold)
            toolbarTitle.lineBreakMode = .byTruncatingTail
            toolbarTitle.setContentHuggingPriority(.defaultHigh, for: .horizontal)
            let item = NSToolbarItem(itemIdentifier: id)
            item.label = "Location"
            item.view = toolbarTitle
            return item
        case .viewMode:
            viewControl.segmentCount = 2
            viewControl.setImage(NSImage(systemSymbolName: "square.grid.2x2", accessibilityDescription: "Icon View"), forSegment: 0)
            viewControl.setImage(NSImage(systemSymbolName: "list.bullet", accessibilityDescription: "List View"), forSegment: 1)
            viewControl.selectedSegment = 1
            viewControl.trackingMode = .selectOne
            viewControl.segmentStyle = .rounded
            viewControl.setEnabled(false, forSegment: 0)
            viewControl.widthAnchor.constraint(equalToConstant: 68).isActive = true
            viewControl.heightAnchor.constraint(equalToConstant: 28).isActive = true
            let item = NSToolbarItem(itemIdentifier: id)
            item.label = "View"
            item.view = viewControl
            return item
        case .actions:
            moreButton.image = NSImage(systemSymbolName: "ellipsis", accessibilityDescription: "Connection Actions")
            moreButton.title = ""
            moreButton.bezelStyle = .texturedRounded
            moreButton.target = self
            moreButton.action = #selector(showConnectionMenu(_:))
            moreButton.widthAnchor.constraint(equalToConstant: 34).isActive = true
            moreButton.heightAnchor.constraint(equalToConstant: 28).isActive = true
            let item = NSToolbarItem(itemIdentifier: id)
            item.label = "Actions"
            item.view = moreButton
            return item
        case .addConnection:
            headerAddButton.image = NSImage(systemSymbolName: "plus", accessibilityDescription: "Add Connection")
            headerAddButton.title = ""
            headerAddButton.bezelStyle = .texturedRounded
            headerAddButton.target = self
            headerAddButton.action = #selector(addConnection)
            headerAddButton.widthAnchor.constraint(equalToConstant: 34).isActive = true
            headerAddButton.heightAnchor.constraint(equalToConstant: 28).isActive = true
            let item = NSToolbarItem(itemIdentifier: id)
            item.label = "Add"
            item.view = headerAddButton
            return item
        case .search:
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

    private var selected: SavedConnection? {
        guard let selectedID else { return nil }
        return visibleDevices.first { $0.id == selectedID }
    }

    private func reload(select preferredID: String? = nil, message: String? = nil) {
        guard !busy else { return }
        setBusy(true, subtitle: message ?? "Loading saved processors…")
        api.load { [weak self] result in
            guard let self else { return }
            switch result {
            case let .success(context):
                self.context = context
                let wanted = preferredID ?? self.selectedID
                self.selectedID = context.devices.contains(where: { $0.id == wanted }) ? wanted : context.devices.first?.id
                self.filterAndReload()
                self.setBusy(false, subtitle: self.showingNetwork ? "Saved processors on this Mac" : "\(context.devices.count) saved processor\(context.devices.count == 1 ? "" : "s")")
            case let .failure(error):
                self.setBusy(false, subtitle: "Connections unavailable")
                self.presentError(error.localizedDescription)
            }
        }
    }

    private func filterAndReload() {
        let query = searchField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let source = showingNetwork ? discovered.map {
            SavedConnection(id: "network:\($0.host)", name: $0.name, host: $0.host, port: 6201, terminalPort: 23, hasCode: false, model: ProcessorModel(rawValue: $0.model) ?? .auto)
        } : context.devices
        visibleDevices = query.isEmpty ? source : source.filter {
            $0.name.localizedStandardContains(query) || $0.host.localizedStandardContains(query)
        }
        tableView.reloadData()
        if let selectedID, let row = visibleDevices.firstIndex(where: { $0.id == selectedID }) {
            tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            tableView.scrollRowToVisible(row)
        } else {
            tableView.deselectAll(nil)
        }
        updateInspector()
    }

    private func setBusy(_ value: Bool, subtitle: String) {
        busy = value
        subtitleLabel.stringValue = subtitle
        if value { progress.startAnimation(nil) } else { progress.stopAnimation(nil) }
        for control in [allButton, networkButton, addButton, headerAddButton, moreButton, removeButton, connectButton, editButton, searchField] {
            control.isEnabled = !value
        }
        updateInspector()
    }

    private func updateInspector() {
        guard let selected else {
            detailName.stringValue = context.devices.isEmpty ? "Add your first Optimod" : "Select a connection"
            detailAddress.stringValue = context.devices.isEmpty ? "Save its name, address, ports and access code once." : ""
            detailCode.stringValue = ""
            connectButton.isEnabled = false
            editButton.isEnabled = false
            removeButton.isEnabled = false
            return
        }
        detailName.stringValue = selected.name
        detailAddress.stringValue = "\(selected.processorModel.label) · \(selected.host):\(selected.port) · Status port \(selected.terminalPort)"
        detailCode.stringValue = selected.hasCode ? "Access code  ••••••••" : "Access code not saved"
        let active = context.connected && context.activeID == selected.id
        let network = selected.id.hasPrefix("network:")
        connectButton.title = network ? "Add…" : active ? "Disconnect" : "Connect"
        connectButton.isEnabled = !busy && (network || selected.hasCode || active)
        editButton.title = network ? "Add…" : "Edit…"
        editButton.isEnabled = !busy
        removeButton.isEnabled = !busy && !network
    }

    func numberOfRows(in tableView: NSTableView) -> Int { visibleDevices.count }

    func tableView(_ tableView: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
        ConnectionTableRowView(alternating: row.isMultiple(of: 2) == false)
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard visibleDevices.indices.contains(row) else { return nil }
        let device = visibleDevices[row]
        let cell = NSTableCellView()
        let image = NSImageView(image: NSImage(systemSymbolName: "server.rack", accessibilityDescription: nil) ?? NSImage())
        image.contentTintColor = .secondaryLabelColor
        image.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 18, weight: .regular)
        image.translatesAutoresizingMaskIntoConstraints = false

        let name = NSTextField(labelWithString: device.name)
        name.font = .systemFont(ofSize: 13, weight: .medium)
        name.lineBreakMode = .byTruncatingTail
        let address = NSTextField(labelWithString: "\(device.processorModel.label) · \(device.host)")
        address.font = .systemFont(ofSize: 11)
        address.textColor = .secondaryLabelColor
        address.lineBreakMode = .byTruncatingMiddle
        let labels = NSStackView(views: [name, address])
        labels.orientation = .vertical
        labels.alignment = .leading
        labels.spacing = 2

        let active = context.connected && context.activeID == device.id
        let network = device.id.hasPrefix("network:")
        let status = NSTextField(labelWithString: network ? "Discovered on Network" : active ? "Connected" : device.hasCode ? "Disconnected" : "Access code required")
        status.font = .systemFont(ofSize: 13, weight: active ? .medium : .regular)
        status.textColor = active ? .systemGreen : .secondaryLabelColor

        let info = NSButton()
        info.image = NSImage(systemSymbolName: "info", accessibilityDescription: "Connection Info")
        info.title = ""
        info.bezelStyle = .texturedRounded
        info.contentTintColor = .labelColor
        info.target = self
        info.action = #selector(editFromInfo(_:))
        info.identifier = NSUserInterfaceItemIdentifier(device.id)
        info.toolTip = "Edit \(device.name)"

        let rowView = NSStackView(views: [image, labels, NSView(), status, info])
        rowView.orientation = .horizontal
        rowView.alignment = .centerY
        rowView.spacing = 12
        rowView.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(rowView)
        NSLayoutConstraint.activate([
            rowView.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 10),
            rowView.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -8),
            rowView.topAnchor.constraint(equalTo: cell.topAnchor, constant: 5),
            rowView.bottomAnchor.constraint(equalTo: cell.bottomAnchor, constant: -5),
            image.widthAnchor.constraint(equalToConstant: 24),
            image.heightAnchor.constraint(equalToConstant: 24),
            labels.widthAnchor.constraint(greaterThanOrEqualToConstant: 150),
            info.widthAnchor.constraint(equalToConstant: 28),
            info.heightAnchor.constraint(equalToConstant: 28),
        ])
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        let row = tableView.selectedRow
        selectedID = visibleDevices.indices.contains(row) ? visibleDevices[row].id : nil
        updateInspector()
    }

    func controlTextDidChange(_ obj: Notification) { filterAndReload() }

    @objc private func showAllConnections() {
        discovery?.cancel()
        showingNetwork = false
        window?.title = "All Connections"
        toolbarTitle.stringValue = "All Connections"
        let subtitle = "\(context.devices.count) saved processor\(context.devices.count == 1 ? "" : "s")"
        subtitleLabel.stringValue = subtitle
        allButton.state = .on
        networkButton.state = .off
        filterAndReload()
    }

    @objc private func showNetworkConnections() {
        showingNetwork = true
        window?.title = "Network"
        toolbarTitle.stringValue = "Network"
        allButton.state = .off
        networkButton.state = .on
        startDiscovery()
        filterAndReload()
    }

    private func startDiscovery() {
        discovery?.cancel()
        discovered.removeAll()
        filterAndReload()
        subtitleLabel.stringValue = "Searching the local network for supported OPTIMOD processors…"
        let scanner = OptimodNetworkDiscovery()
        discovery = scanner
        scanner.discover(found: { [weak self] device in
            guard let self, self.showingNetwork else { return }
            if !self.discovered.contains(where: { $0.host == device.host }) {
                self.discovered.append(device)
                self.discovered.sort { $0.host.localizedStandardCompare($1.host) == .orderedAscending }
                self.filterAndReload()
            }
        }, completion: { [weak self, weak scanner] in
            guard let self, self.showingNetwork, self.discovery === scanner else { return }
            self.subtitleLabel.stringValue = self.discovered.isEmpty
                ? "No supported OPTIMOD processors found"
                : "\(self.discovered.count) OPTIMOD processor\(self.discovered.count == 1 ? "" : "s") found"
        })
    }

    @objc private func addConnection() { presentEditor(connection: nil, connectAfterSave: false) }

    @objc private func showConnectionMenu(_ sender: NSButton) {
        let menu = NSMenu()
        let connect = menu.addItem(withTitle: connectButton.title, action: #selector(connectSelected), keyEquivalent: "\r")
        connect.target = self
        connect.isEnabled = selected != nil && !busy && connectButton.isEnabled
        menu.addItem(.separator())
        let edit = menu.addItem(withTitle: "Edit Connection…", action: #selector(editConnection), keyEquivalent: "")
        edit.target = self
        edit.isEnabled = selected != nil && !busy
        let remove = menu.addItem(withTitle: "Remove Connection…", action: #selector(removeConnection), keyEquivalent: "")
        remove.target = self
        remove.isEnabled = selected != nil && !busy
        menu.addItem(.separator())
        let add = menu.addItem(withTitle: "New Optimod…", action: #selector(addConnection), keyEquivalent: "")
        add.target = self
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: sender.bounds.height + 4), in: sender)
    }

    @objc private func toggleSidebar() {
        guard let sidebar = splitViewController.splitViewItems.first else { return }
        sidebar.animator().isCollapsed.toggle()
    }

    @objc private func editFromInfo(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue,
              let connection = visibleDevices.first(where: { $0.id == id }) else { return }
        selectedID = id
        presentEditor(connection: connection, connectAfterSave: false)
    }

    @objc private func editConnection() {
        guard let selected else { return }
        presentEditor(connection: selected, connectAfterSave: false)
    }

    private func presentEditor(connection: SavedConnection?, connectAfterSave: Bool) {
        guard let window, !busy else { return }
        let editor = ConnectionEditorWindowController(connection: connection, connectAfterSave: connectAfterSave)
        connectionEditor = editor
        editor.onCancel = { [weak self, weak editor, weak window] in
            guard let editor, let window else { return }
            window.endSheet(editor.window!)
            self?.connectionEditor = nil
        }
        editor.onSave = { [weak self, weak editor, weak window] name, host, remote, terminal, code, model in
            guard let self, let editor, let window else { return }
            window.endSheet(editor.window!)
            self.connectionEditor = nil
            let edit = ConnectionEdit(
                id: connection?.id.hasPrefix("network:") == true ? nil : connection?.id,
                name: name,
                host: host,
                port: remote,
                terminalPort: terminal,
                model: model
            )
            self.save(edit: edit, code: code, connectAfterSave: connectAfterSave)
        }
        window.beginSheet(editor.window!)
    }

    private func save(edit: ConnectionEdit, code: String, connectAfterSave: Bool) {
        setBusy(true, subtitle: "Saving connection…")
        api.save(edit) { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error):
                self.setBusy(false, subtitle: "Connection not saved")
                self.presentError(error.localizedDescription)
            case let .success(device):
                guard !code.isEmpty else {
                    self.setBusy(false, subtitle: "Connection saved")
                    if connectAfterSave { self.performConnect(device) }
                    else { self.reload(select: device.id, message: "Connection saved") }
                    return
                }
                self.api.saveCode(id: device.id, code: code) { codeResult in
                    switch codeResult {
                    case let .failure(error):
                        self.setBusy(false, subtitle: "Access code not saved")
                        self.presentError(error.localizedDescription)
                        self.reload(select: device.id)
                    case .success:
                        self.setBusy(false, subtitle: "Connection saved")
                        if connectAfterSave { self.performConnect(device) }
                        else { self.reload(select: device.id, message: "Connection saved") }
                    }
                }
            }
        }
    }

    @objc private func connectSelected() {
        guard let selected, !busy else { return }
        let active = context.connected && context.activeID == selected.id
        if active {
            setBusy(true, subtitle: "Disconnecting…")
            api.disconnect { [weak self] result in self?.finishMutation(result, selectedID: selected.id, success: "Disconnected") }
            return
        }
        if !selected.hasCode {
            presentEditor(connection: selected, connectAfterSave: true)
            return
        }
        if context.connected {
            setBusy(true, subtitle: "Switching processor…")
            api.disconnect { [weak self] result in
                guard let self else { return }
                switch result {
                case let .failure(error): self.finishMutation(.failure(error), selectedID: selected.id, success: "")
                case .success: self.performConnect(selected)
                }
            }
        } else {
            performConnect(selected)
        }
    }

    private func performConnect(_ device: SavedConnection) {
        setBusy(true, subtitle: "Connecting to \(device.name)…")
        api.connect(id: device.id) { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error): self.finishMutation(.failure(error), selectedID: device.id, success: "")
            case .success:
                self.setBusy(false, subtitle: "Connected to \(device.name)")
                self.onConnected?()
                self.reload(select: device.id, message: "Connected to \(device.name)")
            }
        }
    }

    @objc private func removeConnection() {
        guard let selected, !selected.id.hasPrefix("network:"), let window, !busy else { return }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Remove \(selected.name)?"
        alert.informativeText = "The saved address and access code will be removed from this Mac."
        alert.addButton(withTitle: "Remove")
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { [weak self] response in
            guard response == .alertFirstButtonReturn, let self else { return }
            let next = ConnectionSelection.next(afterDeleting: selected.id, from: self.context.devices)
            if ConnectionSelection.mustDisconnect(connected: self.context.connected, activeID: self.context.activeID, deletingID: selected.id) {
                self.setBusy(true, subtitle: "Disconnecting…")
                self.api.disconnect { result in
                    switch result {
                    case let .failure(error): self.finishMutation(.failure(error), selectedID: selected.id, success: "")
                    case .success: self.performDelete(selected, nextID: next)
                    }
                }
            } else {
                self.performDelete(selected, nextID: next)
            }
        }
    }

    private func performDelete(_ device: SavedConnection, nextID: String?) {
        setBusy(true, subtitle: "Removing \(device.name)…")
        api.delete(id: device.id) { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error): self.finishMutation(.failure(error), selectedID: device.id, success: "")
            case .success:
                self.selectedID = nextID
                self.setBusy(false, subtitle: "Connection removed")
                self.reload(select: nextID, message: "Connection removed")
            }
        }
    }

    private func finishMutation(_ result: Result<Void, Error>, selectedID: String?, success: String) {
        switch result {
        case let .failure(error):
            setBusy(false, subtitle: "Operation failed")
            presentError(error.localizedDescription)
            reload(select: selectedID)
        case .success:
            setBusy(false, subtitle: success)
            reload(select: selectedID, message: success)
        }
    }

    private func presentError(_ message: String) {
        ErrorLogStore.shared.record(source: "Connections", message: message)
        subtitleLabel.stringValue = "Operation failed · See Error Log"
        NSSound.beep()
    }
}

private final class ConnectionEditorWindowController: NSWindowController {
    private let nameField: NSTextField
    private let hostField: NSTextField
    private let remotePortField: NSTextField
    private let statusPortField: NSTextField
    private let codeField: NSSecureTextField
    private let modelPopup = NSPopUpButton()
    private let validationLabel = NSTextField(wrappingLabelWithString: "")
    private let hasSavedCode: Bool
    var onCancel: (() -> Void)?
    var onSave: ((String, String, Int, Int, String, ProcessorModel) -> Void)?

    init(connection: SavedConnection?, connectAfterSave: Bool) {
        nameField = NSTextField(string: connection?.name ?? "")
        hostField = NSTextField(string: connection?.host ?? "")
        remotePortField = NSTextField(string: String(connection?.port ?? 6201))
        statusPortField = NSTextField(string: String(connection?.terminalPort ?? 23))
        codeField = NSSecureTextField(string: "")
        hasSavedCode = connection?.hasCode == true
        for model in ProcessorModel.allCases {
            modelPopup.addItem(withTitle: model.label)
        }
        modelPopup.selectItem(at: ProcessorModel.allCases.firstIndex(of: connection?.processorModel ?? .auto) ?? 0)

        let editorWindow = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 620, height: 470),
            styleMask: [.titled, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        editorWindow.title = connection == nil ? "New Optimod" : "Connection Details"
        editorWindow.titleVisibility = .hidden
        editorWindow.titlebarAppearsTransparent = true
        editorWindow.titlebarSeparatorStyle = .none
        editorWindow.isMovableByWindowBackground = true
        super.init(window: editorWindow)

        let content = NSView()
        let heading = NSTextField(labelWithString: connection == nil ? "New Optimod" : "Connection Details")
        heading.font = .systemFont(ofSize: 22, weight: .semibold)
        let detail = NSTextField(wrappingLabelWithString: "The address, ports and access code are stored in this app on this Mac.")
        detail.textColor = .secondaryLabelColor
        detail.font = .systemFont(ofSize: 12.5)
        let titleStack = NSStackView(views: [heading, detail])
        titleStack.orientation = .vertical
        titleStack.alignment = .leading
        titleStack.spacing = 4
        titleStack.translatesAutoresizingMaskIntoConstraints = false

        codeField.placeholderString = hasSavedCode ? "••••••••" : "Access code"
        for field in [nameField, hostField, remotePortField, statusPortField, codeField] {
            field.controlSize = .large
            field.font = .systemFont(ofSize: 14)
            field.widthAnchor.constraint(equalToConstant: 292).isActive = true
        }
        remotePortField.formatter = ConnectionPortFormatter()
        statusPortField.formatter = ConnectionPortFormatter()
        modelPopup.controlSize = .large
        modelPopup.font = .systemFont(ofSize: 14)
        modelPopup.widthAnchor.constraint(equalToConstant: 292).isActive = true

        let nameCard = makeCard(rows: [("Name", nameField)])
        let connectionCard = makeCard(rows: [
            ("Processor", modelPopup),
            ("IP Address", hostField),
            ("PC Remote Port", remotePortField),
            ("Status Port", statusPortField),
            ("Access Code", codeField),
        ])
        nameCard.translatesAutoresizingMaskIntoConstraints = false
        connectionCard.translatesAutoresizingMaskIntoConstraints = false

        validationLabel.textColor = .systemRed
        validationLabel.font = .systemFont(ofSize: 11.5)
        validationLabel.isHidden = true
        validationLabel.translatesAutoresizingMaskIntoConstraints = false

        let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancelEditor))
        cancel.bezelStyle = .rounded
        cancel.keyEquivalent = "\u{1b}"
        let save = NSButton(title: connectAfterSave ? "Add & Connect" : connection == nil ? "Add" : "Done", target: self, action: #selector(saveEditor))
        save.bezelStyle = .rounded
        save.keyEquivalent = "\r"
        let buttons = NSStackView(views: [NSView(), cancel, save])
        buttons.orientation = .horizontal
        buttons.spacing = 10
        buttons.translatesAutoresizingMaskIntoConstraints = false

        let separator = NSBox()
        separator.boxType = .separator
        separator.translatesAutoresizingMaskIntoConstraints = false

        for view in [titleStack, nameCard, connectionCard, validationLabel, separator, buttons] { content.addSubview(view) }
        editorWindow.contentView = content
        NSLayoutConstraint.activate([
            titleStack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 38),
            titleStack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -38),
            titleStack.topAnchor.constraint(equalTo: content.topAnchor, constant: 48),
            nameCard.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 38),
            nameCard.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -38),
            nameCard.topAnchor.constraint(equalTo: titleStack.bottomAnchor, constant: 22),
            connectionCard.leadingAnchor.constraint(equalTo: nameCard.leadingAnchor),
            connectionCard.trailingAnchor.constraint(equalTo: nameCard.trailingAnchor),
            connectionCard.topAnchor.constraint(equalTo: nameCard.bottomAnchor, constant: 14),
            validationLabel.leadingAnchor.constraint(equalTo: nameCard.leadingAnchor, constant: 12),
            validationLabel.trailingAnchor.constraint(equalTo: nameCard.trailingAnchor, constant: -12),
            validationLabel.topAnchor.constraint(equalTo: connectionCard.bottomAnchor, constant: 10),
            separator.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            separator.bottomAnchor.constraint(equalTo: buttons.topAnchor, constant: -14),
            buttons.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 30),
            buttons.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -30),
            buttons.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20),
        ])
        editorWindow.initialFirstResponder = nameField
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    private func makeCard(rows: [(String, NSView)]) -> NSView {
        let card = NSBox()
        card.boxType = .custom
        card.cornerRadius = 13
        card.borderWidth = 0
        card.fillColor = NSColor(name: NSColor.Name("OpenOptimodEditorCard")) { appearance in
            appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
                ? NSColor(calibratedWhite: 0.19, alpha: 1)
                : NSColor(calibratedWhite: 0.965, alpha: 1)
        }
        let gridRows: [[NSView]] = rows.map { title, field in
            let label = NSTextField(labelWithString: title)
            label.font = .systemFont(ofSize: 14)
            return [label, field]
        }
        let grid = NSGridView(views: gridRows)
        grid.rowSpacing = 10
        grid.columnSpacing = 18
        grid.column(at: 0).xPlacement = .leading
        grid.column(at: 1).xPlacement = .fill
        grid.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(grid)
        NSLayoutConstraint.activate([
            grid.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 18),
            grid.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -18),
            grid.topAnchor.constraint(equalTo: card.topAnchor, constant: 14),
            grid.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -14),
        ])
        return card
    }

    @objc private func cancelEditor() { onCancel?() }

    @objc private func saveEditor() {
        let name = nameField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let host = hostField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let code = codeField.stringValue
        guard !name.isEmpty, !host.isEmpty,
              let remote = Int(remotePortField.stringValue), (1...65_535).contains(remote),
              let terminal = Int(statusPortField.stringValue), (1...65_535).contains(terminal),
              hasSavedCode || !code.isEmpty
        else {
            validationLabel.stringValue = "Enter a name, IP address, valid ports and an access code."
            validationLabel.isHidden = false
            NSSound.beep()
            return
        }
        let model = ProcessorModel.allCases.indices.contains(modelPopup.indexOfSelectedItem)
            ? ProcessorModel.allCases[modelPopup.indexOfSelectedItem]
            : .auto
        onSave?(name, host, remote, terminal, code, model)
    }
}

private final class ConnectionPortFormatter: NumberFormatter, @unchecked Sendable {
    override init() {
        super.init()
        numberStyle = .none
        minimum = 1
        maximum = 65_535
        allowsFloats = false
        generatesDecimalNumbers = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }
}

private final class SidebarSourceButton: NSButton {
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

private final class ConnectionTableRowView: NSTableRowView {
    private let alternating: Bool

    init(alternating: Bool) {
        self.alternating = alternating
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func drawBackground(in dirtyRect: NSRect) {
        guard alternating, !isSelected else { return }
        subtleFill.setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 2, dy: 2), xRadius: 9, yRadius: 9).fill()
    }

    override func drawSelection(in dirtyRect: NSRect) {
        guard selectionHighlightStyle != .none else { return }
        let color = isEmphasized
            ? NSColor.selectedContentBackgroundColor
            : NSColor.unemphasizedSelectedContentBackgroundColor
        color.setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 2, dy: 2), xRadius: 9, yRadius: 9).fill()
    }

    private var subtleFill: NSColor {
        NSColor(name: NSColor.Name("OpenOptimodConnectionRowFill")) { appearance in
            appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
                ? NSColor(calibratedWhite: 0.22, alpha: 1)
                : NSColor(calibratedWhite: 0.955, alpha: 1)
        }
    }
}

private extension NSToolbarItem.Identifier {
    static let browserTitle = NSToolbarItem.Identifier("OpenOptimodConnectionsTitle")
    static let viewMode = NSToolbarItem.Identifier("OpenOptimodConnectionsViewMode")
    static let actions = NSToolbarItem.Identifier("OpenOptimodConnectionsActions")
    static let addConnection = NSToolbarItem.Identifier("OpenOptimodConnectionsAdd")
    static let search = NSToolbarItem.Identifier("OpenOptimodConnectionsSearch")
}

enum ConnectionsPresentation {
    static let defaultSize = NSSize(width: 920, height: 500)
    static let minimumSize = NSSize(width: 780, height: 430)
    static let sidebarWidth: CGFloat = 220
    static let minimumSidebarWidth: CGFloat = 200
    static let maximumSidebarWidth: CGFloat = 260
    static let rowHeight: CGFloat = 52
}
