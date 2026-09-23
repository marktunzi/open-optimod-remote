import AppKit
import Foundation

final class SystemSettingsWindowController: NSWindowController, NSWindowDelegate {
    private let api = SystemSettingsAPI()
    private let rootViewController = NSViewController()
    private var tabController = NSTabViewController()
    private var context: SystemSettingsContext?
    private var busy = false
    private var statusText = "Loading settings…"
    private var detailText = ""
    private var selectedSections: [String: String] = [:]

    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1180, height: 820),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Optimod 5700i Settings"
        window.minSize = NSSize(width: 980, height: 650)
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.backgroundColor = SettingsPalette.canvas
        super.init(window: window)
        window.delegate = self
        configureRoot()
        window.setFrameAutosaveName("OpenOptimodRemoteSystemSettingsWindow")
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

    private func configureRoot() {
        guard let window else { return }
        let root = SettingsCanvasView()
        root.translatesAutoresizingMaskIntoConstraints = false
        rootViewController.view = root
        window.contentViewController = rootViewController

        tabController.tabStyle = .toolbar
        rootViewController.addChild(tabController)
        let tabs = tabController.view
        tabs.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(tabs)
        NSLayoutConstraint.activate([
            tabs.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            tabs.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            tabs.topAnchor.constraint(equalTo: root.topAnchor),
            tabs.bottomAnchor.constraint(equalTo: root.bottomAnchor),
        ])
        updateWindowSubtitle()
        renderTabs(selectedTitle: nil)
    }

    private func reload(after message: String? = nil) {
        guard !busy else { return }
        setBusy(true, status: message ?? "Loading settings…")
        api.load { [weak self] result in
            guard let self else { return }
            switch result {
            case let .success(context):
                self.context = context
                self.setBusy(false, status: context.snapshot.connected ? (context.snapshot.writeEnabled ? "Connected · Changes enabled" : "Connected · Read only") : "Not connected")
                self.setDetail(context.snapshot.connected ? context.snapshot.firmware : (context.snapshot.error ?? "Connect in the main 5700i window."))
                self.renderTabs(selectedTitle: self.selectedTabTitle)
            case let .failure(error):
                self.setBusy(false, status: "Settings unavailable")
                self.setDetail(error.localizedDescription)
                self.renderTabs(selectedTitle: self.selectedTabTitle)
            }
        }
    }

    private var selectedTabTitle: String? {
        guard tabController.selectedTabViewItemIndex >= 0,
              tabController.selectedTabViewItemIndex < tabController.tabViewItems.count
        else { return nil }
        return tabController.tabViewItems[tabController.selectedTabViewItemIndex].label
    }

    private func setBusy(_ value: Bool, status: String) {
        busy = value
        statusText = status
        updateWindowSubtitle()
    }

    private func setDetail(_ detail: String) {
        detailText = detail
        updateWindowSubtitle()
    }

    private func updateWindowSubtitle() {
        window?.subtitle = [statusText, detailText].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    private func renderTabs(selectedTitle: String?) {
        let previous = tabController
        let replacement = NSTabViewController()
        replacement.tabStyle = .toolbar
        rootViewController.addChild(replacement)
        replacement.view.translatesAutoresizingMaskIntoConstraints = false
        rootViewController.view.addSubview(replacement.view)
        NSLayoutConstraint.activate([
            replacement.view.leadingAnchor.constraint(equalTo: rootViewController.view.leadingAnchor),
            replacement.view.trailingAnchor.constraint(equalTo: rootViewController.view.trailingAnchor),
            replacement.view.topAnchor.constraint(equalTo: rootViewController.view.topAnchor),
            replacement.view.bottomAnchor.constraint(equalTo: rootViewController.view.bottomAnchor),
        ])

        for tab in SystemSettingsSpecification.tabs {
            let controller = SystemSettingsTabController(
                tab: tab,
                context: context,
                busy: busy,
                selectedSectionTitle: selectedSections[tab.title],
                sectionChanged: { [weak self] sectionTitle in
                    self?.selectedSections[tab.title] = sectionTitle
                },
                commit: { [weak self] item, index, requestedText in
                    self?.commit(item: item, index: index, requestedText: requestedText)
                },
                validationError: { [weak self] message in self?.showValidationError(message) }
            )
            controller.title = tab.title
            let tabItem = NSTabViewItem(viewController: controller)
            tabItem.label = tab.title
            tabItem.image = NSImage(systemSymbolName: tab.symbolName, accessibilityDescription: tab.title)
                ?? NSImage(systemSymbolName: "gearshape", accessibilityDescription: tab.title)
            replacement.addTabViewItem(tabItem)
        }
        if let selectedTitle,
           let index = replacement.tabViewItems.firstIndex(where: { $0.label == selectedTitle }) {
            replacement.selectedTabViewItemIndex = index
        }
        previous.view.removeFromSuperview()
        previous.removeFromParent()
        tabController = replacement
    }

    private func commit(item: SettingsItem, index: Int, requestedText: String?) {
        guard !busy, let context, let fieldName = item.deviceField else { return }
        if case let .unavailable(reason) = item.writePolicy {
            showValidationError(reason)
            return
        }
        setBusy(true, status: "Applying \(item.label)…")
        api.change(context: context, fieldName: fieldName, index: index, requestedText: requestedText) { [weak self] result in
            guard let self else { return }
            self.busy = false
            switch result {
            case .success:
                self.reload(after: "Confirmed · Refreshing \(item.label)…")
            case let .failure(error):
                self.setBusy(false, status: "Change not confirmed")
                self.setDetail(error.localizedDescription)
                self.showValidationError(error.localizedDescription)
                self.reload()
            }
        }
    }

    private func showValidationError(_ message: String) {
        ErrorLogStore.shared.record(source: "System Settings", message: message)
        setBusy(false, status: "Setting not applied · See Error Log")
        NSSound.beep()
    }
}

private final class SystemSettingsTabController: NSViewController {
    private let tab: SettingsTab
    private let context: SystemSettingsContext?
    private let busy: Bool
    private let selectedSectionTitle: String?
    private let sectionChanged: (String) -> Void
    private let commit: (SettingsItem, Int, String?) -> Void
    private let validationError: (String) -> Void
    private let sectionTabs = NSTabView()
    private var sectionSelector: NSSegmentedControl?

    init(
        tab: SettingsTab,
        context: SystemSettingsContext?,
        busy: Bool,
        selectedSectionTitle: String?,
        sectionChanged: @escaping (String) -> Void,
        commit: @escaping (SettingsItem, Int, String?) -> Void,
        validationError: @escaping (String) -> Void
    ) {
        self.tab = tab
        self.context = context
        self.busy = busy
        self.selectedSectionTitle = selectedSectionTitle
        self.sectionChanged = sectionChanged
        self.commit = commit
        self.validationError = validationError
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func loadView() {
        let root = SettingsCanvasView()
        if tab.displaysAllSections {
            let scroll = makeAllSectionsScroll()
            scroll.translatesAutoresizingMaskIntoConstraints = false
            root.addSubview(scroll)
            NSLayoutConstraint.activate([
                scroll.leadingAnchor.constraint(equalTo: root.leadingAnchor),
                scroll.trailingAnchor.constraint(equalTo: root.trailingAnchor),
                scroll.topAnchor.constraint(equalTo: root.topAnchor),
                scroll.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            ])
            view = root
            return
        }
        let selector = NSSegmentedControl(
            labels: tab.sections.map(\.title),
            trackingMode: .selectOne,
            target: self,
            action: #selector(sectionSelected(_:))
        )
        selector.segmentStyle = .rounded
        selector.translatesAutoresizingMaskIntoConstraints = false
        sectionSelector = selector

        sectionTabs.tabViewType = .noTabsNoBorder
        sectionTabs.translatesAutoresizingMaskIntoConstraints = false
        for section in tab.sections {
            let item = NSTabViewItem(identifier: section.title)
            item.label = section.title
            item.view = makeSectionScroll(section)
            sectionTabs.addTabViewItem(item)
        }

        let resolved = SettingsSectionSelection.resolve(preferred: selectedSectionTitle, in: tab.sections)
        if let resolved, let index = tab.sections.firstIndex(where: { $0.title == resolved }) {
            selector.selectedSegment = index
            sectionTabs.selectTabViewItem(at: index)
            sectionChanged(resolved)
        }

        root.addSubview(selector)
        root.addSubview(sectionTabs)
        NSLayoutConstraint.activate([
            selector.topAnchor.constraint(equalTo: root.topAnchor, constant: 20),
            selector.centerXAnchor.constraint(equalTo: root.centerXAnchor),
            selector.leadingAnchor.constraint(greaterThanOrEqualTo: root.leadingAnchor, constant: 24),
            selector.trailingAnchor.constraint(lessThanOrEqualTo: root.trailingAnchor, constant: -24),
            sectionTabs.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            sectionTabs.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            sectionTabs.topAnchor.constraint(equalTo: selector.bottomAnchor, constant: 14),
            sectionTabs.bottomAnchor.constraint(equalTo: root.bottomAnchor),
        ])
        view = root
    }

    private func makeAllSectionsScroll() -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.drawsBackground = true
        scroll.backgroundColor = SettingsPalette.canvas

        let document = FlippedView()
        document.translatesAutoresizingMaskIntoConstraints = false
        scroll.documentView = document
        let sections = NSStackView()
        sections.orientation = .vertical
        sections.alignment = .leading
        sections.spacing = 18
        sections.translatesAutoresizingMaskIntoConstraints = false
        for section in tab.sections {
            let view = SettingsSectionView(
                section: section,
                context: context,
                busy: busy,
                showsHeading: true,
                commit: commit,
                validationError: validationError
            )
            view.translatesAutoresizingMaskIntoConstraints = false
            sections.addArrangedSubview(view)
            view.widthAnchor.constraint(equalTo: sections.widthAnchor).isActive = true
        }
        document.addSubview(sections)
        let clip = scroll.contentView
        NSLayoutConstraint.activate([
            document.leadingAnchor.constraint(equalTo: clip.leadingAnchor),
            document.trailingAnchor.constraint(equalTo: clip.trailingAnchor),
            document.topAnchor.constraint(equalTo: clip.topAnchor),
            document.widthAnchor.constraint(equalTo: clip.widthAnchor),
            sections.leadingAnchor.constraint(equalTo: document.leadingAnchor, constant: 28),
            sections.trailingAnchor.constraint(equalTo: document.trailingAnchor, constant: -28),
            sections.topAnchor.constraint(equalTo: document.topAnchor, constant: 24),
            sections.bottomAnchor.constraint(equalTo: document.bottomAnchor, constant: -28),
        ])
        return scroll
    }

    private func makeSectionScroll(_ section: SettingsSection) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.drawsBackground = true
        scroll.backgroundColor = SettingsPalette.canvas

        let document = FlippedView()
        document.translatesAutoresizingMaskIntoConstraints = false
        scroll.documentView = document
        let sectionView = SettingsSectionView(
            section: section,
            context: context,
            busy: busy,
            showsHeading: false,
            commit: commit,
            validationError: validationError
        )
        sectionView.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(sectionView)
        let clip = scroll.contentView
        NSLayoutConstraint.activate([
            document.leadingAnchor.constraint(equalTo: clip.leadingAnchor),
            document.trailingAnchor.constraint(equalTo: clip.trailingAnchor),
            document.topAnchor.constraint(equalTo: clip.topAnchor),
            document.widthAnchor.constraint(equalTo: clip.widthAnchor),
            sectionView.leadingAnchor.constraint(equalTo: document.leadingAnchor, constant: 28),
            sectionView.trailingAnchor.constraint(equalTo: document.trailingAnchor, constant: -28),
            sectionView.topAnchor.constraint(equalTo: document.topAnchor, constant: 18),
            sectionView.bottomAnchor.constraint(equalTo: document.bottomAnchor, constant: -28),
        ])
        return scroll
    }

    @objc private func sectionSelected(_ sender: NSSegmentedControl) {
        guard tab.sections.indices.contains(sender.selectedSegment) else { return }
        sectionTabs.selectTabViewItem(at: sender.selectedSegment)
        sectionChanged(tab.sections[sender.selectedSegment].title)
    }
}

private final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

private final class SettingsCanvasView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        SettingsPalette.canvas.setFill()
        dirtyRect.fill()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }
}

private final class SettingsSectionView: NSBox {
    init(
        section: SettingsSection,
        context: SystemSettingsContext?,
        busy: Bool,
        showsHeading: Bool = true,
        commit: @escaping (SettingsItem, Int, String?) -> Void,
        validationError: @escaping (String) -> Void
    ) {
        super.init(frame: .zero)
        titlePosition = .noTitle
        boxType = .custom
        borderColor = .clear
        borderWidth = 0
        cornerRadius = 8
        fillColor = SettingsPalette.card
        contentViewMargins = NSSize(width: 14, height: 14)

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 5
        stack.translatesAutoresizingMaskIntoConstraints = false
        contentView?.addSubview(stack)
        if let contentView {
            NSLayoutConstraint.activate([
                stack.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
                stack.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
                stack.topAnchor.constraint(equalTo: contentView.topAnchor),
                stack.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            ])
        }

        if showsHeading {
            let heading = NSTextField(labelWithString: section.title)
            heading.font = .systemFont(ofSize: 13, weight: .semibold)
            heading.textColor = .secondaryLabelColor
            stack.addArrangedSubview(heading)
            let headingSeparator = NSBox()
            headingSeparator.boxType = .separator
            stack.addArrangedSubview(headingSeparator)
            headingSeparator.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        }

        for (index, item) in section.items.enumerated() {
            if index > 0 {
                let separator = NSBox()
                separator.boxType = .separator
                separator.translatesAutoresizingMaskIntoConstraints = false
                stack.addArrangedSubview(separator)
                separator.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
            }
            let row = SettingsControlRow(item: item, context: context, busy: busy, commit: commit, validationError: validationError)
            row.translatesAutoresizingMaskIntoConstraints = false
            stack.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }
}

private final class SettingsControlRow: NSView, NSTextFieldDelegate {
    private let item: SettingsItem
    private let context: SystemSettingsContext?
    private let commitHandler: (SettingsItem, Int, String?) -> Void
    private let validationError: (String) -> Void
    private let field: DeviceFieldPayload?
    private let definition: DeviceDefinitionPayload?
    private let editable: Bool
    private var optionIndices: [Int?] = []
    private var optionTextValues: [String?] = []
    private var rangeEntries: [(index: Int, value: Double)] = []
    private var slider: NSSlider?
    private var valueField: NSTextField?
    private var toggle: NSSwitch?
    private var popup: NSPopUpButton?
    private var segmented: NSSegmentedControl?
    private var stepper: NSStepper?
    private var offRangeEnabled = false
    private var offRangeMinimum = 0.0
    private var offRangeMaximum = 0.0
    private var offRangeUnit = ""
    private var sampleDelay: SampleDelayPayload?

    init(
        item: SettingsItem,
        context: SystemSettingsContext?,
        busy: Bool,
        commit: @escaping (SettingsItem, Int, String?) -> Void,
        validationError: @escaping (String) -> Void
    ) {
        self.item = item
        self.context = context
        commitHandler = commit
        self.validationError = validationError
        field = item.deviceField.flatMap { context?.snapshot.system?.fields[$0] }
        definition = item.deviceField.flatMap { context?.definitions[$0] }
        if case .unavailable = item.writePolicy {
            editable = false
        } else {
            editable = context?.editable == true && field != nil && !busy
        }
        super.init(frame: .zero)
        build()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    private func build() {
        let title = NSTextField(wrappingLabelWithString: item.label)
        title.font = .systemFont(ofSize: 12.5, weight: .medium)
        title.maximumNumberOfLines = 2
        title.lineBreakMode = .byWordWrapping
        title.setContentCompressionResistancePriority(.required, for: .horizontal)
        title.widthAnchor.constraint(equalToConstant: SettingsLayoutMetrics.labelWidth).isActive = true

        let control = makeControl()
        control.setContentHuggingPriority(.required, for: .horizontal)
        control.setContentCompressionResistancePriority(.required, for: .horizontal)

        let main = NSStackView(views: [title, control, NSView()])
        main.orientation = .horizontal
        main.alignment = .centerY
        main.spacing = 12
        main.translatesAutoresizingMaskIntoConstraints = false

        let outer = NSStackView()
        outer.orientation = .vertical
        outer.alignment = .leading
        outer.spacing = 3
        outer.translatesAutoresizingMaskIntoConstraints = false
        outer.addArrangedSubview(main)
        main.widthAnchor.constraint(equalTo: outer.widthAnchor).isActive = true

        let explanation = [policyExplanation, item.note].compactMap { $0 }.joined(separator: " ")
        if !explanation.isEmpty {
            let note = NSTextField(wrappingLabelWithString: explanation)
            note.font = .systemFont(ofSize: 10.5)
            note.textColor = .tertiaryLabelColor
            outer.addArrangedSubview(note)
            note.leadingAnchor.constraint(equalTo: outer.leadingAnchor, constant: SettingsLayoutMetrics.labelWidth + 12).isActive = true
            note.trailingAnchor.constraint(lessThanOrEqualTo: outer.trailingAnchor).isActive = true
        }
        addSubview(outer)
        NSLayoutConstraint.activate([
            outer.leadingAnchor.constraint(equalTo: leadingAnchor),
            outer.trailingAnchor.constraint(equalTo: trailingAnchor),
            outer.topAnchor.constraint(equalTo: topAnchor, constant: 7),
            outer.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -7),
        ])
    }

    private var policyExplanation: String? {
        if case let .unavailable(reason) = item.writePolicy { return reason }
        if context?.snapshot.connected == false { return "Connect in the main window to load and edit this value." }
        if context?.snapshot.writeEnabled == false { return "The active connection is read only." }
        if field == nil { return "This setting is not present in the connected unit's System document." }
        if item.writePolicy == .profile && definition == nil { return "This control is unavailable for the connected firmware profile." }
        return nil
    }

    private func makeControl() -> NSView {
        switch item.control {
        case let .range(minimum, maximum, _, unit):
            return makeRange(minimum: minimum, maximum: maximum, unit: unit)
        case let .toggle(offLabel, onLabel, offAliases, onAliases):
            return makeToggle(offLabel: offLabel, onLabel: onLabel, offAliases: offAliases, onAliases: onAliases)
        case let .segmented(options):
            return makeSegmented(options: options)
        case let .picker(options, includesDevicePresets):
            return makePicker(options: dynamicOptions(options, includePresets: includesDevicePresets))
        case let .text(maxLength, validation):
            return makeText(maxLength: maxLength, validation: validation)
        case let .integer(minimum, maximum, unit):
            return makeInteger(minimum: minimum, maximum: maximum, unit: unit)
        case let .offRange(minimum, maximum, step, unit, useSlider):
            return makeOffRange(minimum: minimum, maximum: maximum, step: step, unit: unit, useSlider: useSlider)
        case .delayTrim:
            return makeDelayTrim()
        case .alternateFrequency:
            return makeAlternateFrequency()
        }
    }

    private func makeRange(minimum: Double, maximum: Double, unit: String) -> NSView {
        let stack = horizontalControlStack()
        rangeEntries = definition?.numericEntries(minimum: minimum, maximum: maximum) ?? []
        let slider = NSSlider(value: 0, minValue: 0, maxValue: Double(max(0, rangeEntries.count - 1)), target: self, action: #selector(rangeChanged(_:)))
        slider.isContinuous = false
        slider.isEnabled = editable && !rangeEntries.isEmpty
        slider.widthAnchor.constraint(equalToConstant: SettingsLayoutMetrics.sliderWidth).isActive = true
        if let field, let position = rangeEntries.firstIndex(where: { $0.index == field.index }) {
            slider.integerValue = position
        }
        self.slider = slider

        let numeric = numericTextField(value: field?.value.numeric, width: 72)
        numeric.target = self
        numeric.action = #selector(rangeTextChanged(_:))
        numeric.delegate = self
        numeric.isEnabled = editable && !rangeEntries.isEmpty
        valueField = numeric
        stack.addArrangedSubview(slider)
        stack.addArrangedSubview(numeric)
        stack.addArrangedSubview(unitLabel(unit))
        return stack
    }

    private func makeToggle(offLabel: String, onLabel: String, offAliases: [String], onAliases: [String]) -> NSView {
        let stack = horizontalControlStack()
        let control = NSSwitch()
        control.target = self
        control.action = #selector(toggleChanged(_:))
        let offIndex = definition?.deviceIndex(matching: offAliases)
        let onIndex = definition?.deviceIndex(matching: onAliases)
        optionIndices = [offIndex, onIndex]
        if field?.index == onIndex { control.state = .on }
        else { control.state = .off }
        control.isEnabled = editable && offIndex != nil && onIndex != nil
        toggle = control
        let state = NSTextField(labelWithString: control.state == .on ? onLabel : offLabel)
        state.tag = 7001
        state.textColor = .secondaryLabelColor
        stack.addArrangedSubview(control)
        stack.addArrangedSubview(state)
        stack.addArrangedSubview(NSView())
        return stack
    }

    private func makeSegmented(options: [SettingsOption]) -> NSView {
        let control = NSSegmentedControl(labels: options.map(\.label), trackingMode: .selectOne, target: self, action: #selector(segmentChanged(_:)))
        control.segmentStyle = .rounded
        optionIndices = options.map { definition?.deviceIndex(matching: $0.aliases) }
        for index in options.indices {
            control.setEnabled(editable && optionIndices[index] != nil, forSegment: index)
            if field?.index == optionIndices[index] { control.selectedSegment = index }
        }
        control.setContentHuggingPriority(.defaultLow, for: .horizontal)
        segmented = control
        return control
    }

    private func makePicker(options: [SettingsOption]) -> NSView {
        let control = NSPopUpButton()
        control.target = self
        control.action = #selector(pickerChanged(_:))
        if item.writePolicy == .typedText, let field {
            optionIndices = options.map { _ in field.index }
            optionTextValues = options.map { $0.aliases.first ?? $0.label }
        } else {
            optionIndices = options.map { definition?.deviceIndex(matching: $0.aliases) }
            optionTextValues = options.map { _ in nil }
        }
        let currentText = field?.value.value.text ?? field?.value.displayText
        for (position, option) in options.enumerated() {
            control.addItem(withTitle: option.label)
            control.item(at: position)?.isEnabled = editable && optionIndices[position] != nil
            if item.writePolicy == .typedText {
                if option.aliases.contains(where: { $0.caseInsensitiveCompare(currentText ?? "") == .orderedSame }) {
                    control.selectItem(at: position)
                }
            } else if field?.index == optionIndices[position] {
                control.selectItem(at: position)
            }
        }
        if control.indexOfSelectedItem < 0, let current = field?.value.displayText {
            control.insertItem(withTitle: current, at: 0)
            optionIndices.insert(field?.index, at: 0)
            optionTextValues.insert(item.writePolicy == .typedText ? current : nil, at: 0)
            control.selectItem(at: 0)
        }
        control.isEnabled = field != nil
        control.widthAnchor.constraint(equalToConstant: SettingsLayoutMetrics.pickerWidth).isActive = true
        popup = control
        return control
    }

    private func makeText(maxLength: Int?, validation: SettingsTextValidation) -> NSView {
        let field = NSTextField(string: self.field?.value.value.text ?? "")
        field.placeholderString = validation == .ipAddress ? "0.0.0.0" : ""
        field.target = self
        field.action = #selector(textChanged(_:))
        field.delegate = self
        field.cell?.sendsActionOnEndEditing = true
        field.isEnabled = editable && item.writePolicy == .typedText
        field.identifier = NSUserInterfaceItemIdentifier("text:\(validation):\(maxLength.map(String.init) ?? "none")")
        field.widthAnchor.constraint(equalToConstant: SettingsLayoutMetrics.textFieldWidth).isActive = true
        valueField = field
        return field
    }

    private func makeInteger(minimum: Int, maximum: Int, unit: String) -> NSView {
        let stack = horizontalControlStack()
        let current: Double? = item.writePolicy == .typedText ? field?.value.value.numeric : field?.value.numeric
        let text = numericTextField(value: current, width: 96)
        text.target = self
        text.action = #selector(integerChanged(_:))
        text.delegate = self
        text.cell?.sendsActionOnEndEditing = true
        text.identifier = NSUserInterfaceItemIdentifier("integer:\(minimum):\(maximum)")
        text.isEnabled = editable
        let stepper = NSStepper()
        stepper.minValue = Double(minimum)
        stepper.maxValue = Double(maximum)
        stepper.increment = 1
        stepper.doubleValue = current ?? Double(minimum)
        stepper.target = self
        stepper.action = #selector(integerStepperChanged(_:))
        stepper.isEnabled = editable
        self.stepper = stepper
        valueField = text
        stack.addArrangedSubview(text)
        stack.addArrangedSubview(stepper)
        stack.addArrangedSubview(unitLabel(unit))
        stack.addArrangedSubview(NSView())
        return stack
    }

    private func makeOffRange(minimum: Double, maximum: Double, step: Double, unit: String, useSlider: Bool) -> NSView {
        offRangeMinimum = minimum
        offRangeMaximum = maximum
        offRangeUnit = unit
        rangeEntries = definition?.numericEntries(minimum: minimum, maximum: maximum) ?? []
        let isOff = field.map { definition?.values[safe: $0.index]?.displayText.caseInsensitiveCompare("Off") == .orderedSame } ?? true
        offRangeEnabled = !isOff
        let storedKey = "OpenOptimodRemote.LastNumeric.\(item.deviceField ?? item.label)"
        let currentNumeric = field?.value.numeric
        if let currentNumeric { UserDefaults.standard.set(currentNumeric, forKey: storedKey) }
        let stored = currentNumeric ?? (UserDefaults.standard.object(forKey: storedKey) as? Double) ?? max(minimum, min(maximum, 0))

        let stack = horizontalControlStack()
        let enabled = NSSwitch()
        enabled.state = offRangeEnabled ? .on : .off
        enabled.target = self
        enabled.action = #selector(offRangeToggleChanged(_:))
        enabled.isEnabled = editable && definition?.deviceIndex(matching: ["Off"]) != nil && !rangeEntries.isEmpty
        toggle = enabled
        stack.addArrangedSubview(enabled)

        if useSlider {
            let slider = NSSlider(value: 0, minValue: 0, maxValue: Double(max(0, rangeEntries.count - 1)), target: self, action: #selector(offRangeSliderChanged(_:)))
            slider.isContinuous = false
            slider.isEnabled = editable && !rangeEntries.isEmpty
            if let position = rangeEntries.firstIndex(where: { abs($0.value - stored) < 0.000_001 }) { slider.integerValue = position }
            slider.widthAnchor.constraint(equalToConstant: SettingsLayoutMetrics.sliderWidth).isActive = true
            self.slider = slider
            stack.addArrangedSubview(slider)
        }

        let numeric = numericTextField(value: stored, width: 72)
        numeric.target = self
        numeric.action = #selector(offRangeTextChanged(_:))
        numeric.delegate = self
        numeric.isEnabled = editable && !rangeEntries.isEmpty
        valueField = numeric
        stack.addArrangedSubview(numeric)
        if !useSlider {
            let stepper = NSStepper()
            stepper.minValue = minimum
            stepper.maxValue = maximum
            stepper.increment = step
            stepper.doubleValue = stored
            stepper.target = self
            stepper.action = #selector(offRangeStepperChanged(_:))
            stepper.isEnabled = editable
            self.stepper = stepper
            stack.addArrangedSubview(stepper)
        }
        stack.addArrangedSubview(unitLabel(unit))
        return stack
    }

    private func makeDelayTrim() -> NSView {
        let stack = horizontalControlStack()
        sampleDelay = definition?.sampleDelay
        let milliseconds: Double
        if let delay = sampleDelay, let field {
            milliseconds = Double(field.index + delay.offset) / Double(delay.rate) * 1000
        } else {
            milliseconds = 0
        }
        let text = numericTextField(value: milliseconds, width: 96)
        text.target = self
        text.action = #selector(delayChanged(_:))
        text.delegate = self
        text.isEnabled = editable && sampleDelay != nil
        let stepper = NSStepper()
        if let delay = sampleDelay {
            stepper.minValue = Double(delay.offset) / Double(delay.rate) * 1000
            stepper.maxValue = Double(delay.maxIndex + delay.offset) / Double(delay.rate) * 1000
            stepper.increment = 1000 / Double(delay.rate)
        }
        stepper.doubleValue = milliseconds
        stepper.target = self
        stepper.action = #selector(delayStepperChanged(_:))
        stepper.isEnabled = editable && sampleDelay != nil
        valueField = text
        self.stepper = stepper
        stack.addArrangedSubview(text)
        stack.addArrangedSubview(stepper)
        stack.addArrangedSubview(unitLabel("ms"))
        stack.addArrangedSubview(NSView())
        return stack
    }

    private func makeAlternateFrequency() -> NSView {
        var options = [SettingsOption("None", aliases: ["0", "None"])]
        for tenth in 876...1079 {
            let value = Double(tenth) / 10
            let text = String(format: "%.1f MHz", value)
            options.append(SettingsOption(text, aliases: [text, String(format: "%.1f", value)]))
        }
        return makePicker(options: options)
    }

    private func dynamicOptions(_ options: [SettingsOption], includePresets: Bool) -> [SettingsOption] {
        guard includePresets, let context else { return options }
        let factory = context.snapshot.presets.filter { $0.kind.caseInsensitiveCompare("Factory") == .orderedSame }
            .map { SettingsOption("Factory · \($0.name)", aliases: [$0.name]) }
        let user = context.snapshot.presets.filter { $0.kind.caseInsensitiveCompare("User") == .orderedSame }
            .map { SettingsOption("User · \($0.name)", aliases: [$0.name]) }
        return options + factory + user
    }

    private func horizontalControlStack() -> NSStackView {
        let stack = NSStackView()
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 8
        return stack
    }

    private func numericTextField(value: Double?, width: CGFloat) -> NSTextField {
        let field = NSTextField(string: value.map { DeviceValuePayload.format($0) } ?? "")
        field.alignment = .right
        field.font = .monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        field.cell?.sendsActionOnEndEditing = true
        field.widthAnchor.constraint(equalToConstant: width).isActive = true
        return field
    }

    private func unitLabel(_ unit: String) -> NSTextField {
        let label = NSTextField(labelWithString: unit)
        label.textColor = .secondaryLabelColor
        label.widthAnchor.constraint(equalToConstant: unit.isEmpty ? 1 : 38).isActive = true
        return label
    }

    @objc private func rangeChanged(_ sender: NSSlider) {
        guard rangeEntries.indices.contains(sender.integerValue) else { return }
        let entry = rangeEntries[sender.integerValue]
        valueField?.stringValue = DeviceValuePayload.format(entry.value)
        commitHandler(item, entry.index, nil)
    }

    @objc private func rangeTextChanged(_ sender: NSTextField) {
        guard let value = parseNumber(sender.stringValue),
              let entry = rangeEntries.first(where: { abs($0.value - value) < 0.000_001 })
        else {
            validationError("Enter a value supported by the processor within this range.")
            return
        }
        if let position = rangeEntries.firstIndex(where: { $0.index == entry.index }) { slider?.integerValue = position }
        commitHandler(item, entry.index, nil)
    }

    @objc private func toggleChanged(_ sender: NSSwitch) {
        let position = sender.state == .on ? 1 : 0
        guard optionIndices.indices.contains(position), let index = optionIndices[position] else { return }
        commitHandler(item, index, nil)
    }

    @objc private func segmentChanged(_ sender: NSSegmentedControl) {
        guard optionIndices.indices.contains(sender.selectedSegment), let index = optionIndices[sender.selectedSegment] else { return }
        commitHandler(item, index, nil)
    }

    @objc private func pickerChanged(_ sender: NSPopUpButton) {
        guard optionIndices.indices.contains(sender.indexOfSelectedItem), let index = optionIndices[sender.indexOfSelectedItem], editable else {
            if case let .unavailable(reason) = item.writePolicy { validationError(reason) }
            return
        }
        let requested = optionTextValues.indices.contains(sender.indexOfSelectedItem)
            ? optionTextValues[sender.indexOfSelectedItem]
            : nil
        commitHandler(item, index, requested)
    }

    @objc private func textChanged(_ sender: NSTextField) {
        let text = sender.stringValue
        guard case let .text(maxLength, validation) = item.control else { return }
        if let maxLength, text.count > maxLength {
            validationError("\(item.label) may contain at most \(maxLength) characters.")
            return
        }
        guard text.unicodeScalars.allSatisfy({ $0.value >= 0x20 && $0.value <= 0x7e }), !text.contains(";"), !text.contains("<"), !text.contains(">") else {
            validationError("Use printable text without semicolons or angle brackets.")
            return
        }
        if validation == .ipAddress && !isIPv4(text) {
            validationError("Enter a valid IPv4 address.")
            return
        }
        if validation == .hexadecimal && (!text.isEmpty && text.range(of: "^[0-9A-Fa-f]+$", options: .regularExpression) == nil) {
            validationError("Enter hexadecimal characters only.")
            return
        }
        guard let field else { return }
        commitHandler(item, field.index, text)
    }

    @objc private func integerChanged(_ sender: NSTextField) {
        guard case let .integer(minimum, maximum, _) = item.control,
              let value = Int(sender.stringValue), (minimum...maximum).contains(value), let field
        else {
            validationError("Enter a whole number within the displayed range.")
            return
        }
        stepper?.integerValue = value
        if item.writePolicy == .typedText { commitHandler(item, field.index, String(value)) }
        else if let index = definition?.deviceIndex(numericValue: Double(value)) { commitHandler(item, index, nil) }
        else { validationError("This value is not present in the verified firmware profile.") }
    }

    @objc private func integerStepperChanged(_ sender: NSStepper) {
        valueField?.integerValue = sender.integerValue
        integerChanged(valueField!)
    }

    @objc private func offRangeToggleChanged(_ sender: NSSwitch) {
        guard let definition else { return }
        if sender.state == .off {
            guard let index = definition.deviceIndex(matching: ["Off"]) else { return }
            offRangeEnabled = false
            commitHandler(item, index, nil)
        } else {
            offRangeEnabled = true
            commitOffRangeValue()
        }
    }

    @objc private func offRangeSliderChanged(_ sender: NSSlider) {
        guard rangeEntries.indices.contains(sender.integerValue) else { return }
        let entry = rangeEntries[sender.integerValue]
        valueField?.stringValue = DeviceValuePayload.format(entry.value)
        storeOffRange(entry.value)
        if offRangeEnabled { commitHandler(item, entry.index, nil) }
    }

    @objc private func offRangeTextChanged(_ sender: NSTextField) {
        commitOffRangeValue()
    }

    @objc private func offRangeStepperChanged(_ sender: NSStepper) {
        valueField?.doubleValue = sender.doubleValue
        commitOffRangeValue()
    }

    private func commitOffRangeValue() {
        guard let text = valueField?.stringValue,
              let value = parseNumber(text), value >= offRangeMinimum, value <= offRangeMaximum,
              let entry = rangeEntries.first(where: { abs($0.value - value) < 0.000_001 })
        else {
            validationError("Enter a supported value between \(DeviceValuePayload.format(offRangeMinimum)) and \(DeviceValuePayload.format(offRangeMaximum)) \(offRangeUnit).")
            return
        }
        storeOffRange(value)
        if let position = rangeEntries.firstIndex(where: { $0.index == entry.index }) { slider?.integerValue = position }
        if offRangeEnabled { commitHandler(item, entry.index, nil) }
    }

    private func storeOffRange(_ value: Double) {
        UserDefaults.standard.set(value, forKey: "OpenOptimodRemote.LastNumeric.\(item.deviceField ?? item.label)")
    }

    @objc private func delayChanged(_ sender: NSTextField) {
        guard let delay = sampleDelay, let milliseconds = parseNumber(sender.stringValue) else {
            validationError("Enter a valid delay in milliseconds.")
            return
        }
        let raw = milliseconds / 1000 * Double(delay.rate) - Double(delay.offset)
        let index = Int(raw.rounded())
        guard index >= 0, index <= delay.maxIndex, abs(raw - Double(index)) < 0.000_1 else {
            validationError("Use a delay on the 15.625 µs device grid.")
            return
        }
        stepper?.doubleValue = milliseconds
        commitHandler(item, index, nil)
    }

    @objc private func delayStepperChanged(_ sender: NSStepper) {
        valueField?.stringValue = DeviceValuePayload.format(sender.doubleValue, decimals: 6)
        delayChanged(valueField!)
    }

    private func parseNumber(_ text: String) -> Double? {
        Double(text.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: ",", with: "."))
    }

    private func isIPv4(_ text: String) -> Bool {
        let parts = text.split(separator: ".", omittingEmptySubsequences: false)
        return parts.count == 4 && parts.allSatisfy { part in
            guard !part.isEmpty, part.count <= 3, part.allSatisfy(\.isNumber), let number = Int(part) else { return false }
            return (0...255).contains(number)
        }
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

private enum SettingsPalette {
    static let canvas = NSColor(name: NSColor.Name("OpenOptimodSettingsCanvas")) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(calibratedWhite: 0.105, alpha: 1)
            : NSColor(calibratedWhite: 0.945, alpha: 1)
    }

    static let card = NSColor(name: NSColor.Name("OpenOptimodSettingsCard")) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(calibratedWhite: 0.17, alpha: 1)
            : .white
    }
}
