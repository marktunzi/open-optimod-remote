import AppKit

/// The original 5700i client's Tools dialogs (Security, Automation) as native
/// windows. They read and write the unit's own files through the service
/// (`/api/security`, `/api/automation`), which confirms every save by reading
/// the file back; the setting switches go through `/api/change`.
private enum ToolsPalette {
    static let canvas = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(calibratedWhite: 0.105, alpha: 1) : NSColor(calibratedWhite: 0.945, alpha: 1)
    }
    static let card = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? NSColor(calibratedWhite: 0.17, alpha: 1) : .white
    }
}

struct ToolError: Error {
    let message: String
    init(_ message: String) { self.message = message }
}

enum ToolsAPI {
    static func get(_ path: String, completion: @escaping (Result<[String: Any], ToolError>) -> Void) {
        let request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!, cachePolicy: .reloadIgnoringLocalCacheData)
        URLSession.shared.dataTask(with: request) { data, response, error in
            let result = Self.decode(data: data, response: response, error: error)
            DispatchQueue.main.async { completion(result) }
        }.resume()
    }

    static func post(_ path: String, _ body: [String: Any], completion: @escaping (Result<[String: Any], ToolError>) -> Void) {
        var request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
        request.timeoutInterval = 60
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        URLSession.shared.dataTask(with: request) { data, response, error in
            let result = Self.decode(data: data, response: response, error: error)
            DispatchQueue.main.async { completion(result) }
        }.resume()
    }

    private static func decode(data: Data?, response: URLResponse?, error: Error?) -> Result<[String: Any], ToolError> {
        if let error { return .failure(ToolError(error.localizedDescription)) }
        let json = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? [:]
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            return .failure(ToolError(json["error"] as? String ?? "The service did not answer"))
        }
        return .success(json)
    }

    /// The current session guards every write carries.
    static func state(_ completion: @escaping (Result<[String: Any], ToolError>) -> Void) { get("state", completion: completion) }

    /// One system setting by its index, with the guards `/api/change` needs.
    static func change(state: [String: Any], name: String, index: Int, completion: @escaping (Result<[String: Any], ToolError>) -> Void) {
        guard let system = state["system"] as? [String: Any], let fields = system["fields"] as? [String: Any], let field = fields[name] else {
            return completion(.failure(ToolError("\(name) is not available")))
        }
        post("change", ["scope": "System", "name": name, "index": index, "expected": field, "requested": NSNull(),
                        "expected_host": state["host"] ?? "", "expected_session": state["session_id"] ?? "",
                        "expected_preset": NSNull()], completion: completion)
    }
}

/// A window with the System Settings canvas, a card and a status line.
class ToolWindowController: NSWindowController, NSWindowDelegate {
    let card = NSStackView()
    let status = NSTextField(wrappingLabelWithString: "")
    var state: [String: Any] = [:]
    private var observer: NSObjectProtocol?
    private var lastRevision: Int?

    init(title: String, size: NSSize) {
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable],
                              backing: .buffered, defer: false)
        window.title = title
        window.minSize = size
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.backgroundColor = ToolsPalette.canvas
        window.isReleasedWhenClosed = false
        super.init(window: window)
        window.delegate = self
        window.setFrameAutosaveName("OpenOptimod.\(title)")

        let box = NSBox()
        box.titlePosition = .noTitle
        box.boxType = .custom
        box.borderWidth = 0
        box.cornerRadius = 8
        box.fillColor = ToolsPalette.card
        box.contentViewMargins = NSSize(width: 16, height: 16)
        card.orientation = .vertical
        card.alignment = .leading
        card.spacing = 10
        card.translatesAutoresizingMaskIntoConstraints = false
        box.contentView?.addSubview(card)
        if let content = box.contentView {
            NSLayoutConstraint.activate([
                card.leadingAnchor.constraint(equalTo: content.leadingAnchor),
                card.trailingAnchor.constraint(equalTo: content.trailingAnchor),
                card.topAnchor.constraint(equalTo: content.topAnchor),
                card.bottomAnchor.constraint(lessThanOrEqualTo: content.bottomAnchor),
            ])
        }
        status.textColor = .secondaryLabelColor
        status.font = .systemFont(ofSize: 12)
        let root = NSView()
        box.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(box)
        NSLayoutConstraint.activate([
            box.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 20),
            box.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -20),
            box.topAnchor.constraint(equalTo: root.topAnchor, constant: 38),
            box.bottomAnchor.constraint(lessThanOrEqualTo: root.bottomAnchor, constant: -20),
        ])
        window.contentView = root
        observer = NotificationCenter.default.addObserver(forName: .deviceSessionChanged, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.window?.isVisible == true else { return }
            let revision = DeviceSessionStore.shared.session?.revision
            if revision != self.lastRevision { self.lastRevision = revision; self.refreshState() }
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    deinit { if let observer { NotificationCenter.default.removeObserver(observer) } }

    func show() {
        // The status line closes the card, under the window's own controls.
        if status.superview == nil {
            card.addArrangedSubview(status)
            status.widthAnchor.constraint(equalTo: card.widthAnchor).isActive = true
        }
        if window?.isVisible != true { window?.center() }
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        refreshState()
        reload()
    }

    func refreshState() {
        ToolsAPI.state { [weak self] result in
            guard let self, case let .success(state) = result else { return }
            self.state = state
            if let host = state["host"] as? String, !host.isEmpty, let title = self.window?.title.components(separatedBy: " – ").first {
                self.window?.title = "\(title) – \(host)"
            }
            self.stateChanged()
        }
    }

    var connected: Bool { state["connected"] as? Bool == true }
    /// The access-level right this window's changes need (`access` of the state).
    var accessRight: String { "" }
    var writable: Bool {
        connected && ((state["access"] as? [String: Any])?[accessRight] as? Bool ?? (state["write_enabled"] as? Bool == true))
    }
    var guards: [String: Any] { ["confirmed": true, "expected_host": state["host"] ?? "", "expected_session": state["session_id"] ?? ""] }
    func systemField(_ name: String) -> [String: Any]? {
        ((state["system"] as? [String: Any])?["fields"] as? [String: Any])?[name] as? [String: Any]
    }

    func reload() {}
    func stateChanged() {}
    /// Test hook (OPEN_OPTIMOD_WINDOW_CONTROL `tool KIND …`).
    func perform(_ words: [String]) {}

    func section(_ title: String) -> NSTextField {
        let heading = NSTextField(labelWithString: title)
        heading.font = .systemFont(ofSize: 13, weight: .semibold)
        heading.textColor = .secondaryLabelColor
        return heading
    }

    func row(_ label: String, _ control: NSView) -> NSStackView {
        let text = NSTextField(labelWithString: label)
        text.alignment = .right
        text.widthAnchor.constraint(equalToConstant: 140).isActive = true
        let stack = NSStackView(views: [text, control])
        stack.spacing = 10
        return stack
    }

    func separator() -> NSBox {
        let line = NSBox()
        line.boxType = .separator
        return line
    }

    func table(columns: [(String, String, CGFloat)], height: CGFloat) -> (NSScrollView, NSTableView) {
        let table = NSTableView()
        table.usesAlternatingRowBackgroundColors = true
        table.style = .inset
        table.allowsEmptySelection = false
        for (id, title, width) in columns {
            let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier(id))
            column.title = title
            column.width = width
            table.addTableColumn(column)
        }
        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        scroll.heightAnchor.constraint(equalToConstant: height).isActive = true
        return (scroll, table)
    }

    func plusMinus(add: Selector, remove: Selector) -> NSSegmentedControl {
        let control = NSSegmentedControl(images: [NSImage(named: NSImage.addTemplateName)!, NSImage(named: NSImage.removeTemplateName)!],
                                         trackingMode: .momentary, target: self, action: #selector(plusMinusPressed(_:)))
        plusMinusActions = (add, remove)
        return control
    }
    private var plusMinusActions: (Selector, Selector)?
    @objc private func plusMinusPressed(_ sender: NSSegmentedControl) {
        guard let (add, remove) = plusMinusActions else { return }
        perform(sender.selectedSegment == 0 ? add : remove, with: nil)
    }

    func setStatus(_ text: String) { status.stringValue = text }
}

// MARK: - Security

final class SecurityWindowController: ToolWindowController, NSTableViewDataSource, NSTableViewDelegate {
    override var accessRight: String { "security" }
    private var passcodes: [(code: String, level: String)] = []
    private var levels: [String] = []
    private var loaded: [(code: String, level: String)] = []
    private var tableView: NSTableView!
    private let levelPopup = NSPopUpButton()
    private let lockPopup = NSPopUpButton()
    private let viewMeters = NSButton(checkboxWithTitle: "View meters and preset while locked", target: nil, action: nil)
    private let saveButton = NSButton(title: "Save to Processor", target: nil, action: nil)
    private let revertButton = NSButton(title: "Revert", target: nil, action: nil)
    private var addRemove: NSSegmentedControl!

    init() {
        super.init(title: "Security", size: NSSize(width: 560, height: 560))
        card.addArrangedSubview(section("Passcodes and Security Levels"))
        let (scroll, table) = table(columns: [("code", "Passcode", 180), ("level", "Authorized to", 300)], height: 170)
        tableView = table
        table.dataSource = self
        table.delegate = self
        card.addArrangedSubview(scroll)
        scroll.widthAnchor.constraint(equalTo: card.widthAnchor).isActive = true
        addRemove = plusMinus(add: #selector(addPasscode), remove: #selector(removePasscode))
        card.addArrangedSubview(addRemove)
        levelPopup.target = self
        levelPopup.action = #selector(levelChosen)
        card.addArrangedSubview(row("Authorized to:", levelPopup))
        let warning = NSTextField(wrappingLabelWithString: "At least one passcode needs access to all screens. The passcode this connection uses stays, with all screens.")
        warning.textColor = .secondaryLabelColor
        warning.font = .systemFont(ofSize: 11)
        card.addArrangedSubview(warning)
        warning.widthAnchor.constraint(equalTo: card.widthAnchor).isActive = true
        saveButton.target = self
        saveButton.action = #selector(save)
        saveButton.keyEquivalent = "\r"
        revertButton.target = self
        revertButton.action = #selector(revert)
        card.addArrangedSubview(NSStackView(views: [revertButton, saveButton]))
        card.addArrangedSubview(separator())
        card.addArrangedSubview(section("Front Panel"))
        lockPopup.addItems(withTitles: ["Off", "1 min", "5 min", "15 min", "30 min", "1 hour", "2 hour", "4 hour", "8 hour"])
        lockPopup.target = self
        lockPopup.action = #selector(lockChosen)
        card.addArrangedSubview(row("Automatic lock:", lockPopup))
        viewMeters.target = self
        viewMeters.action = #selector(viewMetersToggled)
        card.addArrangedSubview(row("", viewMeters))
        updateControls()
    }

    override func reload() {
        setStatus("Reading the processor's passcodes…")
        ToolsAPI.get("security") { [weak self] result in
            guard let self else { return }
            switch result {
            case let .success(data):
                self.levels = data["levels"] as? [String] ?? []
                self.passcodes = (data["passcodes"] as? [[String: Any]] ?? []).map { ($0["code"] as? String ?? "", $0["level"] as? String ?? "") }
                self.loaded = self.passcodes
                self.levelPopup.removeAllItems()
                self.levelPopup.addItems(withTitles: self.levels)
                self.tableView.reloadData()
                if !self.passcodes.isEmpty { self.tableView.selectRowIndexes([0], byExtendingSelection: false) }
                self.setStatus("")
            case let .failure(error): self.setStatus(error.message)
            }
            self.updateControls()
        }
    }

    override func stateChanged() {
        if let lock = systemField("SECURITY TIMEOUT")?["index"] as? Int { lockPopup.selectItem(at: lock) }
        let meters = (systemField("VIEW METERS")?["value"] as? [String: Any])?["value"] as? String
        viewMeters.state = meters == "Yes" ? .on : .off
        updateControls()
    }

    private var dirty: Bool { passcodes.map(\.code) != loaded.map(\.code) || passcodes.map(\.level) != loaded.map(\.level) }

    private func updateControls() {
        let row = tableView?.selectedRow ?? -1
        levelPopup.isEnabled = writable && row >= 0
        if row >= 0, row < passcodes.count { levelPopup.selectItem(withTitle: passcodes[row].level) }
        addRemove?.isEnabled = writable
        saveButton.isEnabled = writable && dirty
        revertButton.isEnabled = dirty
        lockPopup.isEnabled = writable && systemField("SECURITY TIMEOUT") != nil
        viewMeters.isEnabled = writable && systemField("VIEW METERS") != nil
    }

    func numberOfRows(in tableView: NSTableView) -> Int { passcodes.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let text = tableColumn?.identifier.rawValue == "code" ? passcodes[row].code : passcodes[row].level
        let cell = NSTextField(labelWithString: text)
        if tableColumn?.identifier.rawValue == "code" { cell.font = .monospacedSystemFont(ofSize: 12, weight: .regular) }
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) { updateControls() }

    @objc private func addPasscode() {
        let alert = NSAlert()
        alert.messageText = "Add New Passcode"
        alert.informativeText = "Use 1 to 20 letters and digits."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 220, height: 24))
        alert.accessoryView = field
        alert.addButton(withTitle: "Add")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        guard let window else { return }
        alert.beginSheetModal(for: window) { [weak self] response in
            if response == .alertFirstButtonReturn { self?.add(code: field.stringValue) }
        }
    }

    private func add(code raw: String) {
        let code = raw.uppercased()
        guard code.range(of: "^[A-Z0-9]{1,20}$", options: .regularExpression) != nil else { return setStatus("A passcode uses 1 to 20 letters and digits.") }
        guard !passcodes.contains(where: { $0.code == code }) else { return setStatus("This passcode is already programmed.") }
        passcodes.append((code, levels.first ?? "All screens"))
        tableView.reloadData()
        tableView.selectRowIndexes([passcodes.count - 1], byExtendingSelection: false)
        updateControls()
    }

    @objc private func removePasscode() {
        let row = tableView.selectedRow
        guard row >= 0, row < passcodes.count else { return }
        passcodes.remove(at: row)
        tableView.reloadData()
        if !passcodes.isEmpty { tableView.selectRowIndexes([max(0, row - 1)], byExtendingSelection: false) }
        updateControls()
    }

    @objc private func levelChosen() {
        let row = tableView.selectedRow
        guard row >= 0, row < passcodes.count, let level = levelPopup.titleOfSelectedItem else { return }
        passcodes[row].level = level
        tableView.reloadData(forRowIndexes: [row], columnIndexes: [1])
        updateControls()
    }

    @objc private func revert() { reload() }

    @objc private func save() {
        setStatus("Saving the passcodes…")
        saveButton.isEnabled = false
        var body = guards
        body["passcodes"] = passcodes.map { ["code": $0.code, "level": $0.level] }
        ToolsAPI.post("security", body) { [weak self] result in
            switch result {
            case .success: self?.reload(); self?.setStatus("Saved on the processor and read back.")
            case let .failure(error): self?.setStatus(error.message); self?.updateControls()
            }
        }
    }

    @objc private func lockChosen() { writeSetting("SECURITY TIMEOUT", lockPopup.indexOfSelectedItem) }
    @objc private func viewMetersToggled() { writeSetting("VIEW METERS", viewMeters.state == .on ? 1 : 0) }

    private func writeSetting(_ name: String, _ index: Int) {
        ToolsAPI.change(state: state, name: name, index: index) { [weak self] result in
            if case let .failure(error) = result { self?.setStatus(error.message) }
            self?.refreshState()
        }
    }

    override func perform(_ words: [String]) {
        switch words.first {
        case "add" where words.count > 1: add(code: words[1])
        case "select" where words.count > 1:
            if let row = passcodes.firstIndex(where: { $0.code == words[1].uppercased() }) { tableView.selectRowIndexes([row], byExtendingSelection: false) }
        case "level" where words.count > 1: levelPopup.selectItem(at: Int(words[1]) ?? 0); levelChosen()
        case "remove": removePasscode()
        case "revert": revert()
        case "save": save()
        default: break
        }
    }
}

// MARK: - Automation

final class AutomationWindowController: ToolWindowController, NSTableViewDataSource, NSTableViewDelegate {
    override var accessRight: String { "automation" }
    private struct Event {
        var date: Date
        var weekdays: [Bool]
        var daily: Bool
        var action: String
    }

    /// The model's functions come with the events; until then the 5700i's.
    private var functions = ["Bypass", "Exit Test", "Stereo", "Mono-R", "Mono-L", "Mono-Sum", "SSB", "Mod. Reduction 1",
                                    "Mod. Reduction 2", "Exit Mod. Reduction", "Analog Out Delay In", "Analog Out Delay Out",
                                    "Digital Out 1 Delay In", "Digital Out 1 Delay Out", "Digital Out 2 Delay In",
                                    "Digital Out 2 Delay Out", "Stereo Enc Delay In", "Stereo Enc Delay Out"]
    private static let weekdayNames = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
    private var events: [Event] = []
    private var dirty = false
    private var tableView: NSTableView!
    private var addRemove: NSSegmentedControl!
    private let enabled = NSButton(checkboxWithTitle: "Automation Enabled", target: nil, action: nil)
    private let actionPopup = NSPopUpButton()
    private let daily = NSButton(radioButtonWithTitle: "Daily Event", target: nil, action: nil)
    private let dated = NSButton(radioButtonWithTitle: "Date Event", target: nil, action: nil)
    private let timePicker = NSDatePicker()
    private let datePicker = NSDatePicker()
    private var dayBoxes: [NSButton] = []
    private let saveButton = NSButton(title: "Save to Processor", target: nil, action: nil)
    private let revertButton = NSButton(title: "Revert", target: nil, action: nil)
    private let calendar: Calendar = { var c = Calendar(identifier: .gregorian); c.timeZone = TimeZone(identifier: "UTC")!; return c }()

    init() {
        super.init(title: "Automation", size: NSSize(width: 620, height: 600))
        enabled.target = self
        enabled.action = #selector(enabledToggled)
        card.addArrangedSubview(enabled)
        let (scroll, table) = table(columns: [("action", "Action", 230), ("time", "Time", 90), ("occurs", "Occurs as", 200)], height: 170)
        tableView = table
        table.dataSource = self
        table.delegate = self
        card.addArrangedSubview(scroll)
        scroll.widthAnchor.constraint(equalTo: card.widthAnchor).isActive = true
        addRemove = plusMinus(add: #selector(newEvent), remove: #selector(deleteEvent))
        card.addArrangedSubview(addRemove)
        card.addArrangedSubview(separator())
        actionPopup.target = self
        actionPopup.action = #selector(edited)
        card.addArrangedSubview(row("Action:", actionPopup))
        for button in [daily, dated] { button.target = self; button.action = #selector(occursChosen(_:)) }
        card.addArrangedSubview(row("Occurs as:", NSStackView(views: [daily, dated])))
        for picker in [timePicker, datePicker] {
            picker.datePickerStyle = .textFieldAndStepper
            picker.timeZone = calendar.timeZone
            picker.calendar = calendar
            picker.target = self
            picker.action = #selector(edited)
        }
        timePicker.datePickerElements = .hourMinuteSecond
        datePicker.datePickerElements = .yearMonthDay
        card.addArrangedSubview(row("Time:", timePicker))
        card.addArrangedSubview(row("Date:", datePicker))
        dayBoxes = Self.weekdayNames.map { NSButton(checkboxWithTitle: $0, target: self, action: #selector(edited)) }
        let days = NSStackView(views: Array(dayBoxes[0..<4]))
        let days2 = NSStackView(views: Array(dayBoxes[4...]))
        let dayStack = NSStackView(views: [days, days2])
        dayStack.orientation = .vertical
        dayStack.alignment = .leading
        card.addArrangedSubview(row("Weekdays:", dayStack))
        saveButton.target = self
        saveButton.action = #selector(save)
        saveButton.keyEquivalent = "\r"
        revertButton.target = self
        revertButton.action = #selector(revert)
        card.addArrangedSubview(NSStackView(views: [revertButton, saveButton]))
        updateControls()
    }

    override func reload() {
        setStatus("Reading the processor's automation…")
        ToolsAPI.get("automation") { [weak self] result in
            guard let self else { return }
            switch result {
            case let .success(data):
                self.events = (data["events"] as? [[String: Any]] ?? []).map(self.event(from:))
                if let functions = data["functions"] as? [String] {
                    self.functions = functions
                    self.stateChanged()
                }
                self.dirty = false
                self.tableView.reloadData()
                if !self.events.isEmpty { self.tableView.selectRowIndexes([0], byExtendingSelection: false) }
                self.setStatus(self.events.isEmpty ? "No events." : "")
            case let .failure(error): self.setStatus(error.message)
            }
            self.showSelected()
        }
    }

    override func stateChanged() {
        let value = (systemField("AUTOMATION ENABLED")?["value"] as? [String: Any])?["value"] as? String
        enabled.state = value == "Enabled" ? .on : .off
        let presets = (state["presets"] as? [[String: Any]] ?? []).filter { $0["kind"] as? String != "Unsaved" }.compactMap { $0["name"] as? String }
        let current = actionPopup.titleOfSelectedItem
        actionPopup.removeAllItems()
        actionPopup.menu?.addItem(withTitle: "No function", action: nil, keyEquivalent: "")
        actionPopup.menu?.addItem(.separator())
        for name in presets + functions { actionPopup.menu?.addItem(withTitle: name, action: nil, keyEquivalent: "") }
        if let current { actionPopup.selectItem(withTitle: current) }
        updateControls()
    }

    private func event(from data: [String: Any]) -> Event {
        let parts = DateComponents(calendar: calendar, year: data["year"] as? Int, month: data["month"] as? Int, day: data["day"] as? Int,
                                   hour: data["hour"] as? Int, minute: data["minute"] as? Int, second: data["second"] as? Int)
        return Event(date: calendar.date(from: parts) ?? Date(), weekdays: data["weekdays"] as? [Bool] ?? Array(repeating: true, count: 7),
                     daily: data["daily"] as? Bool ?? true, action: data["action"] as? String ?? "")
    }

    private func payload(_ event: Event) -> [String: Any] {
        let c = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: event.date)
        return ["hour": c.hour ?? 0, "minute": c.minute ?? 0, "second": c.second ?? 0, "day": c.day ?? 1, "month": c.month ?? 1,
                "year": c.year ?? 2026, "weekdays": event.weekdays, "daily": event.daily, "action": event.action]
    }

    private var selected: Int? {
        let row = tableView?.selectedRow ?? -1
        return row >= 0 && row < events.count ? row : nil
    }

    private func showSelected() {
        if let i = selected {
            let e = events[i]
            if e.action.isEmpty { actionPopup.selectItem(at: 0) } else { actionPopup.selectItem(withTitle: e.action) }
            daily.state = e.daily ? .on : .off
            dated.state = e.daily ? .off : .on
            timePicker.dateValue = e.date
            datePicker.dateValue = e.date
            for (box, on) in zip(dayBoxes, e.weekdays) { box.state = on ? .on : .off }
        }
        updateControls()
    }

    private func updateControls() {
        let editing = writable && selected != nil
        for control in [actionPopup, daily, dated, timePicker] as [NSControl] { control.isEnabled = editing }
        let isDaily = selected.map { events[$0].daily } ?? true
        datePicker.isEnabled = editing && !isDaily
        dayBoxes.forEach { $0.isEnabled = editing && isDaily }
        enabled.isEnabled = writable && systemField("AUTOMATION ENABLED") != nil
        addRemove?.isEnabled = writable
        saveButton.isEnabled = writable && dirty
        revertButton.isEnabled = dirty
    }

    func numberOfRows(in tableView: NSTableView) -> Int { events.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let e = events[row]
        let c = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: e.date)
        let text: String
        switch tableColumn?.identifier.rawValue {
        case "action": text = e.action.isEmpty ? "No function" : e.action
        case "time": text = String(format: "%02d:%02d:%02d", c.hour ?? 0, c.minute ?? 0, c.second ?? 0)
        default:
            text = e.daily
                ? (e.weekdays.allSatisfy { $0 } ? "Every day" : zip(Self.weekdayNames, e.weekdays).filter(\.1).map { String($0.0.prefix(3)) }.joined(separator: " "))
                : String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0)
        }
        return NSTextField(labelWithString: text)
    }

    func tableViewSelectionDidChange(_ notification: Notification) { showSelected() }

    @objc private func newEvent() {
        // The original client's New Event: daily at 00:00:00 on every weekday.
        var c = calendar.dateComponents([.year, .month, .day], from: Date())
        c.hour = 0; c.minute = 0; c.second = 0
        events.append(Event(date: calendar.date(from: c) ?? Date(), weekdays: Array(repeating: true, count: 7), daily: true, action: ""))
        dirty = true
        tableView.reloadData()
        tableView.selectRowIndexes([events.count - 1], byExtendingSelection: false)
        showSelected()
    }

    @objc private func deleteEvent() {
        guard let i = selected else { return }
        events.remove(at: i)
        dirty = true
        tableView.reloadData()
        if !events.isEmpty { tableView.selectRowIndexes([max(0, i - 1)], byExtendingSelection: false) }
        showSelected()
    }

    @objc private func occursChosen(_ sender: NSButton) {
        daily.state = sender === daily ? .on : .off
        dated.state = sender === dated ? .on : .off
        edited()
    }

    @objc private func edited() {
        guard let i = selected else { return }
        let time = calendar.dateComponents([.hour, .minute, .second], from: timePicker.dateValue)
        var day = calendar.dateComponents([.year, .month, .day], from: datePicker.dateValue)
        day.hour = time.hour; day.minute = time.minute; day.second = time.second
        events[i].date = calendar.date(from: day) ?? events[i].date
        events[i].daily = daily.state == .on
        events[i].weekdays = dayBoxes.map { $0.state == .on }
        events[i].action = actionPopup.indexOfSelectedItem <= 0 ? "" : actionPopup.titleOfSelectedItem ?? ""
        dirty = true
        tableView.reloadData(forRowIndexes: [i], columnIndexes: [0, 1, 2])
        updateControls()
    }

    @objc private func revert() { reload() }

    @objc private func save() {
        guard !events.contains(where: { $0.action.isEmpty }) else { return setStatus("Choose an action for every event.") }
        setStatus("Saving the automation…")
        saveButton.isEnabled = false
        var body = guards
        body["events"] = events.map(payload)
        ToolsAPI.post("automation", body) { [weak self] result in
            switch result {
            case .success: self?.reload(); self?.setStatus("Saved on the processor and read back.")
            case let .failure(error): self?.setStatus(error.message); self?.updateControls()
            }
        }
    }

    @objc private func enabledToggled() {
        ToolsAPI.change(state: state, name: "AUTOMATION ENABLED", index: enabled.state == .on ? 1 : 0) { [weak self] result in
            if case let .failure(error) = result { self?.setStatus(error.message) }
            self?.refreshState()
        }
    }

    override func perform(_ words: [String]) {
        switch words.first {
        case "new": newEvent()
        case "delete": deleteEvent()
        case "action" where words.count > 1: actionPopup.selectItem(withTitle: words.dropFirst().joined(separator: " ")); edited()
        case "time" where words.count > 1:
            let p = words[1].split(separator: ":").compactMap { Int($0) }
            if p.count == 3, var c = Optional(calendar.dateComponents([.year, .month, .day], from: timePicker.dateValue)) {
                c.hour = p[0]; c.minute = p[1]; c.second = p[2]
                if let d = calendar.date(from: c) { timePicker.dateValue = d; edited() }
            }
        case "date":
            occursChosen(dated)
        case "save": save()
        default: break
        }
    }
}

/// The 5500i client's File › Open Setup and Save Setup As: its setup presets
/// (`.orb55setup`) on the processor, as native panels.
enum SetupPresetsPanel {
    static func available(_ adapterID: String?) -> Bool { adapterID?.hasPrefix("pc-remote-5500i-") == true }

    static func open() {
        ToolsAPI.get("setup-presets") { result in
            switch result {
            case let .failure(error): report("Open Setup", error.message)
            case let .success(data):
                let names = data["names"] as? [String] ?? []
                guard !names.isEmpty else { return report("Open Setup", "The processor has no setup presets.") }
                let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 260, height: 26), pullsDown: false)
                popup.addItems(withTitles: names)
                let alert = NSAlert()
                alert.messageText = "Open Setup"
                alert.informativeText = "The processor's system settings are replaced by the chosen setup preset. Inputs, outputs and levels can change."
                alert.accessoryView = popup
                alert.addButton(withTitle: "Open Setup")
                alert.addButton(withTitle: "Cancel")
                guard alert.runModal() == .alertFirstButtonReturn, let name = popup.titleOfSelectedItem else { return }
                perform("open", name)
            }
        }
    }

    static func saveAs() {
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 24))
        field.placeholderString = "Name"
        let alert = NSAlert()
        alert.messageText = "Save Setup As"
        alert.informativeText = "The processor's current system settings are stored on it as a setup preset."
        alert.accessoryView = field
        alert.window.initialFirstResponder = field
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        perform("save_as", field.stringValue.trimmingCharacters(in: .whitespaces))
    }

    /// Also the test hook (`setup open|saveas NAME`), without the panels.
    static func perform(_ action: String, _ name: String) {
        guard let session = DeviceSessionStore.shared.session, let id = session.sessionID else {
            return report(action == "open" ? "Open Setup" : "Save Setup As", "Not connected.")
        }
        let body: [String: Any] = ["action": action, "name": name, "confirmed": true,
                                   "expected_host": session.host, "expected_session": id]
        ToolsAPI.post("setup-presets", body) { result in
            let title = action == "open" ? "Open Setup" : "Save Setup As"
            switch result {
            case .success: lastResult = action == "open" ? "Setup \(name) is in force" : "Saved setup \(name)"
            case let .failure(error): report(title, error.message)
            }
        }
    }

    /// What the last action did, for the test hook's state.
    nonisolated(unsafe) static var lastResult = ""

    private static func report(_ title: String, _ message: String) {
        lastResult = message
        ErrorLogStore.shared.record(source: title, message: message)
        guard ProcessInfo.processInfo.environment["OPEN_OPTIMOD_WINDOW_CONTROL"] == nil else { return }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = title
        alert.informativeText = message
        alert.runModal()
    }
}
