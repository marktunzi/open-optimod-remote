import Foundation

/// System Settings tabs for a model without a hand-made worksheet: every
/// System field the connected processor reports, grouped by function, each
/// with a native control built from that model's own profile. Nothing is taken
/// from another model; a field without a verified definition is shown read-only.
enum ProfileSystemSettings {
    private struct Group {
        let title: String
        let symbol: String
        let sections: [String]
    }

    private static let groups: [Group] = [
        Group(title: "Input", symbol: "arrow.down.circle",
              sections: ["Source Selection", "Analog Input", "Digital Input", "EI1/EI2 Input", "Failover & Silence Detection"]),
        Group(title: "Output", symbol: "arrow.up.circle",
              sections: ["Analog Output", "Digital Output 1", "Digital Output 2", "EO1/EO2 Output", "Monitoring & Composite"]),
        Group(title: "FM", symbol: "dot.radiowaves.left.and.right",
              sections: ["Operating Mode", "Test & Bypass", "Stereo Generator", "Modulation & Carrier", "Loudness Protection", "Processing Structure"]),
        Group(title: "HD & Diversity", symbol: "hifispeaker.2",
              sections: ["HD Output", "Diversity Delay", "Loudness Protection", "HD Processing"]),
        Group(title: "Network & Time", symbol: "network",
              sections: ["IP Configuration", "Service Ports", "Clock & Calendar", "Automatic Clock Set"]),
        Group(title: "Remote Control", symbol: "switch.2",
              sections: ["Automation & Tallies", "Remote Contacts", "Security", "External Interfaces"]),
        Group(title: "RDS", symbol: "antenna.radiowaves.left.and.right",
              sections: ["Programme Service", "Dynamic Text", "Alternate Frequencies", "UECP & Network", "Emergency Alerting"]),
        Group(title: "SNMP", symbol: "server.rack",
              sections: ["Service", "Manager Destinations", "Community Access"]),
        Group(title: "Display", symbol: "display",
              sections: ["Front Panel", "Meter Display"]),
        Group(title: "Identification", symbol: "info.circle",
              sections: ["Station & Hardware", "Kantar", "Ratings Encoder"]),
    ]

    /// The tabs of a model without a hand-made worksheet. The original client's
    /// own Setup pages come first, with its titles, labels and group boxes;
    /// every other System field the processor reports follows in the groups
    /// below. Only fields the processor reports are shown, and only those with
    /// a value the app can show exactly.
    static func tabs(context: SystemSettingsContext?) -> [SettingsTab] {
        guard let context, let fields = context.snapshot.system?.fields else { return [] }
        var placed = Set<String>()
        var tabs: [SettingsTab] = context.pages.compactMap { page in
            let sections = clientSections(page, fields: fields, context: context)
            guard !sections.isEmpty else { return nil }
            sections.forEach { $0.items.forEach { if let name = $0.deviceField { placed.insert(name) } } }
            return SettingsTab(title: page.title, symbolName: symbol(for: page.title), sections: sections)
        }
        var rest: [String: [String: [SettingsItem]]] = [:]
        for name in fields.keys where !placed.contains(name) {
            guard let item = item(for: name, label: nil, field: fields[name], definition: context.definitions[name]) else { continue }
            let group = group(of: name)
            rest[group, default: [:]][section(of: name, in: group), default: []].append(item)
        }
        for group in groups {
            guard let bySection = rest[group.title] else { continue }
            let sections = group.sections.compactMap { title -> SettingsSection? in
                guard let items = bySection[title], !items.isEmpty else { return nil }
                return SettingsSection(title: title, items: items.sorted { $0.label.localizedStandardCompare($1.label) == .orderedAscending })
            }
            guard !sections.isEmpty else { continue }
            if let index = tabs.firstIndex(where: { $0.title == group.title }) {
                let tab = tabs[index]
                tabs[index] = SettingsTab(title: tab.title, symbolName: tab.symbolName, sections: tab.sections + sections)
            } else {
                tabs.append(SettingsTab(title: group.title, symbolName: group.symbol, sections: sections))
            }
        }
        return tabs
    }

    /// A client page's controls in the order of its dialog (column by column),
    /// one section per named group box; controls outside a named box form a
    /// section with the page's title.
    static func clientSections(_ page: SystemLayoutPage, fields: [String: DeviceFieldPayload],
                               context: SystemSettingsContext) -> [SettingsSection] {
        let present = page.controls.filter { fields[$0.name] != nil }
        let ordered = readingOrder(present)
        let boxes = page.groups.filter { !$0.text.trimmingCharacters(in: .whitespaces).isEmpty }
            .sorted { ($0.x, $0.y) < ($1.x, $1.y) }
        var sections: [(title: String, items: [SettingsItem])] = boxes.map { (titleCase($0.text), []) }
        var loose: [SettingsItem] = []
        for control in ordered {
            guard let item = item(for: control.name, label: control.label, field: fields[control.name],
                                  definition: context.definitions[control.name]) else { continue }
            if let box = boxes.firstIndex(where: { contains($0, control) }) {
                sections[box].items.append(item)
            } else {
                loose.append(item)
            }
        }
        let named = sections.filter { !$0.items.isEmpty }.map { SettingsSection(title: $0.title, items: $0.items) }
        return (loose.isEmpty ? [] : [SettingsSection(title: page.title, items: loose)]) + named
    }

    static func readingOrder(_ controls: [SystemLayoutControl]) -> [SystemLayoutControl] {
        var columns: [[SystemLayoutControl]] = []
        for control in controls.sorted(by: { ($0.x, $0.y) < ($1.x, $1.y) }) {
            if let first = columns.last?.first, control.x - first.x <= 40 {
                columns[columns.count - 1].append(control)
            } else {
                columns.append([control])
            }
        }
        return columns.flatMap { $0.sorted { $0.y < $1.y } }
    }

    private static func contains(_ box: SystemLayoutGroup, _ control: SystemLayoutControl) -> Bool {
        control.x >= box.x && control.x <= box.x + box.w && control.y >= box.y && control.y <= box.y + box.h
    }

    private static let acronyms: Set<String> = ["AES", "AES67", "HD", "FM", "RDS", "MPX", "SCA", "IP", "AF", "PS", "RT", "LF", "HF"]

    static func titleCase(_ text: String) -> String {
        text.trimmingCharacters(in: .whitespaces).split(separator: " ").map { word in
            acronyms.contains(word.uppercased()) ? word.uppercased() : word.lowercased().capitalized
        }.joined(separator: " ")
    }

    private static func symbol(for title: String) -> String {
        let table: [(String, String)] = [
            ("AES67", "network"), ("Input", "arrow.down.circle"), ("Output", "arrow.up.circle"),
            ("Stereo Encoder", "dot.radiowaves.left.and.right"), ("Digital MPX", "waveform"), ("HD", "hifispeaker.2"),
            ("Utility", "wrench.and.screwdriver"), ("Test", "checkmark.seal"), ("Remote", "switch.2"),
            ("Network", "network"), ("RDS", "antenna.radiowaves.left.and.right"),
        ]
        return table.first { title.hasPrefix($0.0) }?.1 ?? "gearshape"
    }

    /// One field's native control, from this model's own definition. A
    /// device status is read only; a text value is typed and confirmed by
    /// reading the processor back; a field whose values the profile cannot
    /// name exactly is not shown.
    static func item(for name: String, label clientLabel: String?, field: DeviceFieldPayload?,
                     definition: DeviceDefinitionPayload?) -> SettingsItem? {
        let title = clientLabel.map { $0.trimmingCharacters(in: CharacterSet(charactersIn: " :")) }.flatMap { $0.isEmpty ? nil : $0 }
            ?? label(name)
        if name.hasPrefix("ACTUAL ") {
            return SettingsItem(title, field: name, control: .text(maxLength: nil, validation: .plain),
                                writePolicy: .unavailable("Status reported by the processor."))
        }
        guard let definition, !definition.values.isEmpty else {
            guard field?.value.type == "Text" else { return nil }
            let control = SettingsControlKind.text(maxLength: 255, validation: textValidation(field?.value.value.text))
            if let reason = readOnlyText(name) { return SettingsItem(title, field: name, control: control, writePolicy: .unavailable(reason)) }
            return SettingsItem(title, field: name, control: control, writePolicy: .typedText)
        }
        let numbers = definition.values.compactMap(\.numeric)
        if numbers.count == definition.values.count, let low = numbers.min(), let high = numbers.max() {
            return SettingsItem(title, field: name, control: .range(min: low, max: high, step: 0, unit: definition.unit))
        }
        if definition.values.allSatisfy({ $0.type == "Choice" }) {
            let labels = definition.values.map(\.displayText)
            let options = labels.map { SettingsOption($0) }
            if labels.count == 2, let off = labels.firstIndex(where: offWords.contains) {
                return SettingsItem(title, field: name, control: .toggle(
                    offLabel: labels[off], onLabel: labels[1 - off],
                    offAliases: [labels[off]], onAliases: [labels[1 - off]]))
            }
            if labels.count <= 4, labels.allSatisfy({ $0.count <= 14 }) {
                return SettingsItem(title, field: name, control: .segmented(options: options))
            }
            return SettingsItem(title, field: name, control: .picker(options: options, includesDevicePresets: false))
        }
        // Mixed values (a number range with an Off choice) as a picker of the exact values.
        return SettingsItem(title, field: name, control: .picker(options: definition.values.map { SettingsOption($0.displayText) },
                                                                 includesDevicePresets: false))
    }

    /// Text the remote session must not retype: the connected unit's own
    /// address (a change ends this session; set it on the front panel), and
    /// remote-contact functions, which name one of the unit's own functions.
    static func readOnlyText(_ name: String) -> String? {
        if name.hasPrefix("NETWORK ") { return "Set on the processor's front panel; changing it ends this connection." }
        if name.hasPrefix("REMOTE CONTACT") { return "One of the processor's own functions; set it on the front panel." }
        return nil
    }

    /// An address field holds a dotted IPv4 address on the processor.
    private static func textValidation(_ current: String?) -> SettingsTextValidation {
        guard let current, current.range(of: #"^\d{1,3}(\.\d{1,3}){3}$"#, options: .regularExpression) != nil else { return .plain }
        return .ipAddress
    }

    private static let offWords: Set<String> = ["Off", "OFF", "No", "NO", "Disabled", "Disable", "DISABLED"]

    // The grouping rules of the earlier Setup workspace, by field name.
    private static func matches(_ name: String, _ pattern: String) -> Bool {
        name.range(of: pattern, options: .regularExpression) != nil
    }

    private static let testAndBypass = #"^(TEST (MODE|TONE|BYPASS|CLIP DEFEAT|MODULATION|400HZ TONE)|XTALK TEST|MUTE|OPERATE)$"#
    private static let processingStructure = #"^(2B SWITCH|5B SWITCH|2B/5B SWITCH|STD SWITCH|ULL SWITCH|MX SWITCH|PASSTHRU SW|DRIVE W\d|MIX W\d|CLIP W\d|CLIP DENS THR|AGC MASTER TH|PHASE CORRECT(OR| DEFEAT| XOVER)|SUBHARMONIC|MAG PHASE COMP|B4/5 DELTA REL)$"#
    private static let modulationExtras = #"^(DIGITAL SCA[12] LEVEL|COMPOSITE OSCOMP|MAIN OSCOMP|MPX PWR B5CTRL)$"#
    private static let loudnessExtras = #"^(MPX PWR (REL|SP) CTRL|RESET ITU412)$"#
    private static let automaticClock = #"^(AUTO SET |SET BY |(SUN|MON|TUES|WEDNES|THURS|FRI|SATUR)DAY$|CLOCK CONTROL$)"#
    private static let manualClock = #"^SET (HOUR|MINUTE|SECOND|DAY|MONTH|YEAR)$"#

    static func group(of name: String) -> String {
        if matches(name, #"^(EI[12] |INPUT LEVEL$)"#) || matches(name, "FALLBACK$") { return "Input" }
        if matches(name, #"^(EO[12] |EO LR SWAP|DO LR SWAP|DIGITAL COMP LEVEL$)"#) { return "Output" }
        if matches(name, testAndBypass) || matches(name, processingStructure)
            || matches(name, modulationExtras) || matches(name, loudnessExtras) { return "FM" }
        if matches(name, "^hd ") { return "HD & Diversity" }
        if matches(name, automaticClock) || matches(name, manualClock) { return "Network & Time" }
        if matches(name, "^(PASSCODE ACCESS LEVEL|CURRENT PASSCODES)$") { return "Remote Control" }
        if matches(name, "^(METER OPTION|AGC METER)$") { return "Display" }
        if matches(name, "^RDS ") { return "RDS" }
        if matches(name, "SNMP") { return "SNMP" }
        if matches(name, "^(INPUT A OR D|ACTUAL A OR D|AI |DI |ANALOG FALLBACK|DIGITAL FALLBACK|SILENCE )") { return "Input" }
        if matches(name, "^(AO|DO[12] |PHONES OUT|OUT METER SOURCE|Monitor Mute|COMP[12] OUT|AO PRE-OUT)") { return "Output" }
        if matches(name, "^(ALGORITHM|BYPASS GAIN|PILOT|PRE-E|FM POLARITY|FREQUENCY|MODULATION|MOD |MOD REDUC|CLIP DEFEAT|TEST PILOT|FM BS1770|ITU412)") { return "FM" }
        if matches(name, "^(HD |DIVERSITY|IBOC |BS1770)") { return "HD & Diversity" }
        if matches(name, "^(NETWORK|TERMINAL PORT|TIME |DATE FORMAT|DAYLIGHT|STANDARD MONTH|SYNC PERIOD)") { return "Network & Time" }
        if matches(name, "^(AUTOMATION|REMOTE CONTACT|TALLY|SECURITY|MODEM INIT STRING|STUDIO CHASSIS|INTERFACE TYPE)") { return "Remote Control" }
        if matches(name, "^(CONTRAST|LANGUAGE|LDNES METER|MB GR METER|METER SLEEP|PEAK METER|SCREEN SAVER|SHOW IP|VIEW METERS)") { return "Display" }
        return "Identification"
    }

    static func section(of name: String, in group: String) -> String {
        switch group {
        case "Input":
            if matches(name, "^EI[12] ") { return "EI1/EI2 Input" }
            if matches(name, "^AI ") { return "Analog Input" }
            if matches(name, "^DI ") { return "Digital Input" }
            if matches(name, "FALLBACK|SILENCE") { return "Failover & Silence Detection" }
            return "Source Selection"
        case "Output":
            if matches(name, "^(EO[12] |EO LR SWAP)") { return "EO1/EO2 Output" }
            if matches(name, "^(DO1 |DO LR SWAP)") { return "Digital Output 1" }
            if matches(name, "^DO2 ") { return "Digital Output 2" }
            if matches(name, "^(PHONES|OUT METER|Monitor|COMP|DIGITAL COMP)") { return "Monitoring & Composite" }
            return "Analog Output"
        case "FM":
            if matches(name, testAndBypass) { return "Test & Bypass" }
            if matches(name, processingStructure) { return "Processing Structure" }
            if matches(name, loudnessExtras) { return "Loudness Protection" }
            if matches(name, "^(ALGORITHM|BYPASS)") { return "Operating Mode" }
            if matches(name, "^(PILOT|PRE-E|FM POLARITY)") { return "Stereo Generator" }
            if matches(name, "FM BS1770|ITU412") { return "Loudness Protection" }
            return "Modulation & Carrier"
        case "HD & Diversity":
            if matches(name, "^hd ") { return "HD Processing" }
            if matches(name, "DIVERSITY") { return "Diversity Delay" }
            if matches(name, "BS1770|ITU412") { return "Loudness Protection" }
            return "HD Output"
        case "Network & Time":
            if matches(name, "^NETWORK (?!PORT)") { return "IP Configuration" }
            if matches(name, "PORT$") { return "Service Ports" }
            if matches(name, automaticClock) { return "Automatic Clock Set" }
            return "Clock & Calendar"
        case "Remote Control":
            if matches(name, "^REMOTE CONTACT") { return "Remote Contacts" }
            if matches(name, "^(AUTOMATION|TALLY)") { return "Automation & Tallies" }
            if matches(name, "^(SECURITY|PASSCODE|CURRENT PASSCODES)") { return "Security" }
            return "External Interfaces"
        case "RDS":
            if matches(name, "ALTERNATE FREQUENCY") { return "Alternate Frequencies" }
            if matches(name, "UECP|RDS PORT|SOURCE IP") { return "UECP & Network" }
            if matches(name, "EAS") { return "Emergency Alerting" }
            if matches(name, "DPS|DYNAMIC PS|RADIO TEXT") { return "Dynamic Text" }
            return "Programme Service"
        case "SNMP":
            if matches(name, "ADDRESS|PORT") { return "Manager Destinations" }
            if matches(name, "READ|WRITE") { return "Community Access" }
            return "Service"
        case "Display":
            return matches(name, "METER") ? "Meter Display" : "Front Panel"
        default:
            if matches(name, "^KANTAR|^ACTUAL KANTAR") { return "Kantar" }
            if matches(name, "^RATINGS") { return "Ratings Encoder" }
            return "Station & Hardware"
        }
    }

    private static let exactLabels: [String: String] = [
        "ALGORITHM": "Operating Mode", "INPUT A OR D": "Preferred Input", "ACTUAL A OR D": "Active Input",
        "ANALOG FALLBACK": "Allow Analog Fallback", "DIGITAL FALLBACK": "Allow Digital Fallback",
        "DI ANALOG FALLBACK": "Digital Input Analog Fallback", "AO PRE-OUT": "Analog Pre-processed Output",
        "Monitor Mute": "Mute Monitor Output", "START SNMP": "SNMP Service", "SECURITY ACTIVE": "Remote Security",
        "STATION ID": "Station Name", "RDS PROGRAM ID": "Programme Identification (PI)",
        "RDS PROGRAM NAME": "Program Service Name", "RDS PROGRAM TYPE": "Programme Type (PTY)",
        "RDS DYNAMIC PS": "Dynamic Program Service", "RDS RADIO TEXT": "RadioText",
        "RDS SOURCE IP": "UECP Source Address", "ACTUAL KANTAR": "Kantar Status",
        "LDNES METER UNITS": "Loudness Meter Units", "IBOC METER": "HD Meter Source",
    ]

    static func label(_ name: String) -> String {
        if let exact = exactLabels[name] { return exact }
        var text = name
        for (pattern, replacement) in [("^AI ", "Analog Input "), ("^DI ", "Digital Input "), ("^AO1 ", "Analog Output "),
                                       ("^DO1 ", "Digital Output 1 "), ("^DO2 ", "Digital Output 2 "), ("^RDS ", "")] {
            text = text.replacingOccurrences(of: pattern, with: replacement, options: .regularExpression)
        }
        text = text.replacingOccurrences(of: "_", with: " ")
        for (word, replacement) in [("REF", "Reference"), ("THR", "Threshold"), ("LDNES", "Loudness"), ("BW", "Bandwidth"),
                                    ("DPS", "Dynamic PS"), ("BEGINS", "Starts")] {
            text = text.replacingOccurrences(of: "\\b\(word)\\b", with: replacement, options: .regularExpression)
        }
        text = text.lowercased().capitalized
        for acronym in ["FM", "HD", "RDS", "SNMP", "UECP", "IP", "ID", "CBET", "CSID", "PPM", "EAS", "AGC", "MPX", "AES", "TA", "MP3", "DPS"] {
            text = text.replacingOccurrences(of: "\\b\(acronym.capitalized)\\b", with: acronym, options: .regularExpression)
        }
        return text
    }
}
