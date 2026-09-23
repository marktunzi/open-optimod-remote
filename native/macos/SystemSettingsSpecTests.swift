import Foundation

@main
struct SystemSettingsSpecTests {
    static func main() {
        let tabs = SystemSettingsSpecification.tabs
        precondition(tabs.map(\.title) == [
            "Input", "Output", "Test", "Utility", "Network", "Stereo Encoder",
            "Remote I/O", "HD / Digital Radio", "RDS", "RDS AF",
        ])
        precondition(tabs.allSatisfy { !$0.symbolName.isEmpty })
        precondition(Set(tabs.map(\.symbolName)).count == tabs.count)
        let combinedTabs = Set(tabs.filter(\.displaysAllSections).map(\.title))
        precondition(combinedTabs == ["Input", "Test", "Network", "Stereo Encoder", "HD / Digital Radio"])
        precondition(SettingsSectionSelection.resolve(preferred: "Digital", in: tabs[0].sections) == "Digital")
        precondition(SettingsSectionSelection.resolve(preferred: "Missing", in: tabs[0].sections) == "Source")
        precondition(SettingsSectionSelection.resolve(preferred: nil, in: []) == nil)
        precondition(SettingsLayoutMetrics.textFieldWidth == 260)
        precondition(SettingsLayoutMetrics.pickerWidth == 240)
        precondition(SettingsLayoutMetrics.sliderWidth == 220)

        let output = tabs.first { $0.title == "Output" }!
        let aes1 = output.sections.first { $0.title == "Digital AES 1" }!
        let aes2 = output.sections.first { $0.title == "Digital AES 2" }!
        precondition(aes1.items.count == 8 && aes2.items.count == 8)
        let aes1Sources = pickerLabels(aes1.items.first { $0.label == "Out Source" }!)
        let aes2Sources = pickerLabels(aes2.items.first { $0.label == "Out Source" }!)
        precondition(aes1Sources == ["FM", "FM+Delay", "Monitor", "HD"])
        precondition(aes2Sources == ["FM", "FM+Delay", "Monitor", "HD", "Ratings"])

        let test = tabs.first { $0.title == "Test" }!
        let bypass = test.sections.flatMap(\.items).first { $0.label == "Bypass" }!
        precondition(bypass.deviceField == "ALGORITHM")
        guard case let .toggle(offLabel, onLabel, offAliases, onAliases) = bypass.control else {
            preconditionFailure("Bypass must be a switch")
        }
        precondition(offLabel == "Off" && onLabel == "On")
        precondition(offAliases == ["operate"] && onAliases == ["bypass"])

        let utility = tabs.first { $0.title == "Utility" }!
        let multiplex = utility.sections.flatMap(\.items).first { $0.label == "Multiplex Power Threshold" }!
        guard case let .offRange(minimum, maximum, step, unit, slider) = multiplex.control else {
            preconditionFailure("Multiplex Power Threshold must preserve Off separately from its range")
        }
        precondition(minimum == -2 && maximum == 10 && step == 0.1 && unit == "dB" && slider)

        let remote = tabs.first { $0.title == "Remote I/O" }!
        let gpi = remote.sections.first { $0.title == "GPI Inputs" }!
        precondition(gpi.items.count == 8)
        let functions = pickerLabels(gpi.items[0])
        precondition(functions.count == 27)
        precondition(functions.first == "Any Factory Preset")
        precondition(functions.last == "No Function")
        precondition(gpi.items.allSatisfy { $0.writePolicy == .typedText })

        let unavailable = tabs.flatMap(\.sections).flatMap(\.items).filter {
            if case .unavailable = $0.writePolicy { return true }
            return $0.deviceField == nil
        }
        precondition(unavailable.isEmpty)

        let rds = tabs.first { $0.title == "RDS" }!
        for name in ["Dynamic Program Service Timeout", "TA Timeout"] {
            let item = rds.sections.flatMap(\.items).first { $0.label == name }!
            guard case let .offRange(minimum, maximum, step, unit, slider) = item.control else {
                preconditionFailure("\(name) must preserve Off separately from its numeric value")
            }
            precondition(minimum == 1 && maximum == 255 && step == 1 && unit == "min" && !slider)
        }

        let af = tabs.first { $0.title == "RDS AF" }!
        precondition(af.sections.map { $0.items.count } == [12, 12])
        let afItems = af.sections.flatMap(\.items)
        precondition(afItems.count == 24)
        precondition(afItems.map(\.label) == (1...24).map { "AF \($0)" })
        precondition(afItems.allSatisfy { $0.control == .alternateFrequency })

        let total = tabs.flatMap(\.sections).flatMap(\.items).count
        precondition(total == 134)

        let profileURL = URL(fileURLWithPath: "profiles/5700i/3.0.1.20/parameters.json")
        let profile = try! JSONDecoder().decode(DeviceProfilePayload.self, from: Data(contentsOf: profileURL))
        let system = Dictionary(uniqueKeysWithValues: profile.fields.filter { $0.scope == "System" }.map { ($0.name, $0) })
        let missingProfileFields = tabs
            .flatMap(\.sections)
            .flatMap(\.items)
            .filter { $0.writePolicy == .profile }
            .compactMap(\.deviceField)
            .filter { system[$0] == nil }
        precondition(missingProfileFields.isEmpty, "Missing verified System mappings: \(missingProfileFields)")
        precondition(system["INPUT A OR D"]?.deviceIndex(matching: ["Digital+J.17", "Dig+J17"]) == 2)
        precondition(system["FM BS1770 SAFETY LIMITER"]?.deviceIndex(matching: ["On"]) == 0)
        precondition(system["FM BS1770 SAFETY LIMITER"]?.deviceIndex(matching: ["Off"]) == 1)
        precondition(system["NETWORK PORT"]?.deviceIndex(numericValue: 6201) == 6201)
        precondition(system["DO2 SOURCE"]?.deviceIndex(matching: ["Ratings"]) == nil)
        let afDefinition = system["RDS ALTERNATE FREQUENCY 1"]!
        precondition(afDefinition.deviceIndex(numericValue: 87.6) == 1)
        precondition(afDefinition.deviceIndex(numericValue: 107.9) == 204)
        print("SystemSettingsSpecTests passed: \(tabs.count) tabs, \(total) controls")
    }

    private static func pickerLabels(_ item: SettingsItem) -> [String] {
        guard case let .picker(options, _) = item.control else {
            preconditionFailure("Expected picker for \(item.label)")
        }
        return options.map(\.label)
    }
}
