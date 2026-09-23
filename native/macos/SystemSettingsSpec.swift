import Foundation

enum SettingsLayoutMetrics {
    static let labelWidth = 160.0
    static let sliderWidth = 220.0
    static let textFieldWidth = 260.0
    static let pickerWidth = 240.0
}

enum SettingsTextValidation: Equatable {
    case plain
    case ipAddress
    case hexadecimal
}

struct SettingsOption: Equatable {
    let label: String
    let aliases: [String]

    init(_ label: String, aliases: [String] = []) {
        self.label = label
        self.aliases = aliases.isEmpty ? [label] : aliases
    }
}

enum SettingsControlKind: Equatable {
    case range(min: Double, max: Double, step: Double, unit: String)
    case toggle(offLabel: String, onLabel: String, offAliases: [String], onAliases: [String])
    case segmented(options: [SettingsOption])
    case picker(options: [SettingsOption], includesDevicePresets: Bool)
    case text(maxLength: Int?, validation: SettingsTextValidation)
    case integer(min: Int, max: Int, unit: String)
    case offRange(min: Double, max: Double, step: Double, unit: String, slider: Bool)
    case delayTrim(stepMicroseconds: Double)
    case alternateFrequency
}

enum SettingsWritePolicy: Equatable {
    case profile
    case typedText
    case unavailable(String)
}

struct SettingsItem: Equatable {
    let label: String
    let deviceField: String?
    let control: SettingsControlKind
    let writePolicy: SettingsWritePolicy
    let note: String?

    init(
        _ label: String,
        field: String?,
        control: SettingsControlKind,
        writePolicy: SettingsWritePolicy = .profile,
        note: String? = nil
    ) {
        self.label = label
        self.deviceField = field
        self.control = control
        self.writePolicy = writePolicy
        self.note = note
    }
}

struct SettingsSection: Equatable {
    let title: String
    let items: [SettingsItem]
}

struct SettingsTab: Equatable {
    let title: String
    let symbolName: String
    let sections: [SettingsSection]
    let displaysAllSections: Bool

    init(title: String, symbolName: String, sections: [SettingsSection], displaysAllSections: Bool = false) {
        self.title = title
        self.symbolName = symbolName
        self.sections = sections
        self.displaysAllSections = displaysAllSections
    }
}

enum SettingsSectionSelection {
    static func resolve(preferred: String?, in sections: [SettingsSection]) -> String? {
        if let preferred, sections.contains(where: { $0.title == preferred }) { return preferred }
        return sections.first?.title
    }
}

enum SystemSettingsSpecification {
    static let tabs: [SettingsTab] = [
        input,
        output,
        test,
        utility,
        network,
        stereoEncoder,
        remoteIO,
        digitalRadio,
        rds,
        rdsAF,
    ]

    private static let input = SettingsTab(title: "Input", symbolName: "arrow.down.circle", sections: [
        SettingsSection(title: "Source", items: [
            SettingsItem("Input Source", field: "INPUT A OR D", control: .segmented(options: [
                SettingsOption("Analog"),
                SettingsOption("Digital"),
                SettingsOption("Digital+J.17", aliases: ["Digital+J.17", "Dig+J17"]),
            ])),
        ]),
        SettingsSection(title: "Analog", items: [
            SettingsItem("Ref. Level VU", field: "AI REF LEVEL", control: .range(min: -9, max: 13, step: 0.5, unit: "dBu")),
            SettingsItem("Ref. Level PPM", field: "AI REF PPM LEVEL", control: .range(min: -2, max: 20, step: 0.5, unit: "dBu")),
            SettingsItem("Right Channel Balance", field: "AI BALANCE", control: .range(min: -3, max: 3, step: 0.1, unit: "dB")),
        ]),
        SettingsSection(title: "Digital", items: [
            SettingsItem("Ref. Level VU", field: "DI REF LEVEL", control: .range(min: -30, max: -7, step: 0.5, unit: "dBFS")),
            SettingsItem("Ref. Level PPM", field: "DI REF PPM LEVEL", control: .range(min: -23, max: 0, step: 0.5, unit: "dBFS")),
            SettingsItem("Right Channel Balance", field: "DI BALANCE", control: .range(min: -3, max: 3, step: 0.1, unit: "dB")),
        ]),
    ], displaysAllSections: true)

    private static let output = SettingsTab(title: "Output", symbolName: "arrow.up.circle", sections: [
        SettingsSection(title: "Analog Output", items: [
            SettingsItem("Out Level", field: "AO1 LEVEL", control: .range(min: -6, max: 24, step: 0.1, unit: "dBu")),
            SettingsItem("Pre-Emphasis", field: "AO PRE-OUT", control: .segmented(options: [SettingsOption("Flat"), SettingsOption("Pre-e")])),
            SettingsItem("Out Source", field: "AO1 SOURCE", control: .picker(options: outputSources, includesDevicePresets: false)),
        ]),
        SettingsSection(title: "Digital AES 1", items: digitalOutputItems(prefix: "DO1", includesRatings: false)),
        SettingsSection(title: "Pilot Sync", items: [
            SettingsItem("Pilot Sync", field: "PILOT SYNC", control: .segmented(options: [
                SettingsOption("Ref Input", aliases: ["Ref Input", "REF IN"]),
                SettingsOption("Digital Input", aliases: ["Digital Input", "DIGITAL IN"]),
                SettingsOption("Internal", aliases: ["Internal", "INTERNAL"]),
            ])),
        ]),
        SettingsSection(title: "Digital AES 2", items: digitalOutputItems(prefix: "DO2", includesRatings: true)),
    ])

    private static func digitalOutputItems(prefix: String, includesRatings: Bool) -> [SettingsItem] {
        var sources = outputSources
        if includesRatings {
            sources.append(SettingsOption("Ratings"))
        }
        return [
            SettingsItem("Out Level", field: "\(prefix) LEVEL", control: .range(min: -20, max: 0, step: 0.1, unit: "dBFS")),
            SettingsItem("Pre-Emphasis", field: "\(prefix) PRE EMPH", control: .picker(options: [
                SettingsOption("Flat"), SettingsOption("Pre-e"), SettingsOption("J.17"),
                SettingsOption("Pre+J.17", aliases: ["Pre+J.17", "Pre+J17"]),
            ], includesDevicePresets: false)),
            SettingsItem("Out Source", field: "\(prefix) SOURCE", control: .picker(options: sources, includesDevicePresets: false),
                note: includesRatings ? "Ratings is shown from the 5700i worksheet and remains unavailable when the connected firmware profile does not expose it." : nil),
            SettingsItem("Digital Output Sync", field: "\(prefix) SYNC", control: .segmented(options: [
                SettingsOption("Pilot Sync", aliases: ["Pilot Sync", "PILOT SYNC"]),
                SettingsOption("Digital Input", aliases: ["Digital Input", "DIGITAL IN"]),
            ])),
            SettingsItem("Sample Rate", field: "\(prefix) SAMPLE RATE", control: .picker(options: [
                SettingsOption("32.0 kHz", aliases: ["32.0 kHz", "32kHz"]),
                SettingsOption("44.1 kHz", aliases: ["44.1 kHz", "44.1kHz"]),
                SettingsOption("48.0 kHz", aliases: ["48.0 kHz", "48kHz"]),
                SettingsOption("88.2 kHz", aliases: ["88.2 kHz", "88.2kHz"]),
                SettingsOption("96.0 kHz", aliases: ["96.0 kHz", "96kHz"]),
            ], includesDevicePresets: false)),
            SettingsItem("Dither", field: "\(prefix) DITHER", control: .toggle(offLabel: "Out", onLabel: "In", offAliases: ["Out"], onAliases: ["In"])),
            SettingsItem("Word Length", field: "\(prefix) WORD_LENGTH", control: .picker(options: [14, 16, 18, 20, 24].map {
                SettingsOption("\($0) bit", aliases: ["\($0) bit", "\($0)"])
            }, includesDevicePresets: false)),
            SettingsItem("Format", field: "\(prefix) FORMAT", control: .segmented(options: [SettingsOption("AES"), SettingsOption("SPDIF")])),
        ]
    }

    private static let outputSources = ["FM", "FM+Delay", "Monitor", "HD"].map { SettingsOption($0) }

    private static let test = SettingsTab(title: "Test", symbolName: "waveform", sections: [
        SettingsSection(title: "Bypass", items: [
            SettingsItem("Bypass", field: "ALGORITHM", control: .toggle(offLabel: "Off", onLabel: "On", offAliases: ["operate"], onAliases: ["bypass"])),
            SettingsItem("Gain", field: "BYPASS GAIN", control: .range(min: -18, max: 25, step: 0.1, unit: "dB")),
        ]),
        SettingsSection(title: "Tone", items: [
            SettingsItem("Modulation Level", field: "MOD LEVEL", control: .range(min: 0, max: 121, step: 1, unit: "%")),
            SettingsItem("Frequency", field: "FREQUENCY", control: .range(min: 16, max: 15_000, step: 1, unit: "Hz"), note: "The slider follows the discrete frequencies supported by the processor."),
            SettingsItem("Tone Channels", field: "MOD TYPE", control: .segmented(options: [
                SettingsOption("L+R"), SettingsOption("L-R"),
                SettingsOption("Left only", aliases: ["Left only", "LEFT"]),
                SettingsOption("Right only", aliases: ["Right only", "RIGHT"]),
            ])),
            SettingsItem("19 kHz Pilot", field: "TEST PILOT", control: .toggle(offLabel: "Off", onLabel: "On", offAliases: ["Off"], onAliases: ["On"])),
        ]),
    ], displaysAllSections: true)

    private static let utility = SettingsTab(title: "Utility", symbolName: "wrench.and.screwdriver", sections: [
        SettingsSection(title: "Time Sync", items: [
            SettingsItem("Time Sync Period", field: "SYNC PERIOD", control: .picker(options: ["Off", "1 Hr", "8 Hrs", "24 Hrs"].map { SettingsOption($0) }, includesDevicePresets: false)),
            SettingsItem("Time Sync Protocol", field: "TIME SYNC", control: .segmented(options: [SettingsOption("SNTP"), SettingsOption("Time Pro")])),
            SettingsItem("UTC Offset", field: "TIME OFFSET", control: .range(min: -12, max: 12, step: 1, unit: "h")),
            SettingsItem("Time Server IP Address", field: "TIME SERVER", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
        ]),
        SettingsSection(title: "Processing", items: [
            SettingsItem("Final Clipper active", field: "CLIP DEFEAT", control: .toggle(offLabel: "No", onLabel: "Yes", offAliases: ["Defeat", "No"], onAliases: ["Active", "Yes"])),
            SettingsItem("Processing Pre-Emphasis", field: "PRE-E", control: .segmented(options: [
                SettingsOption("75 µs", aliases: ["75 µs", "75us"]),
                SettingsOption("50 µs", aliases: ["50 µs", "50us"]),
            ])),
            SettingsItem("FM Output Meter", field: "OUT METER SOURCE", control: .segmented(options: [
                SettingsOption("Pre-emphasis", aliases: ["Pre-emphasis", "Pre-emph"]),
                SettingsOption("De-emphasis", aliases: ["De-emphasis", "De-emph"]),
            ])),
            SettingsItem("1770 Loudness Control Threshold", field: "FM BS1770 LDNES CTRL THR", control: .range(min: -31, max: -11, step: 0.1, unit: "dB")),
            SettingsItem("1770 Safety Limiter", field: "FM BS1770 SAFETY LIMITER", control: .toggle(offLabel: "Off", onLabel: "On", offAliases: ["Off"], onAliases: ["On"])),
            SettingsItem("1770 Loudness Meter Units", field: "LDNES METER UNITS", control: .segmented(options: [SettingsOption("LU"), SettingsOption("Lk")])),
        ]),
        SettingsSection(title: "Station", items: [
            SettingsItem("Station ID", field: "STATION ID", control: .text(maxLength: 8, validation: .plain), writePolicy: .typedText),
            SettingsItem("FM Polarity", field: "FM POLARITY", control: .segmented(options: [SettingsOption("Positive"), SettingsOption("Negative")])),
            SettingsItem("Digital Input Analog Fallback", field: "DI ANALOG FALLBACK", control: .toggle(offLabel: "No", onLabel: "Yes", offAliases: ["No"], onAliases: ["Yes"])),
        ]),
        SettingsSection(title: "Modulation", items: [
            SettingsItem("Modulation Reduction 1", field: "MOD REDUCE 1", control: .range(min: -20, max: 0, step: 0.5, unit: "%")),
            SettingsItem("Modulation Reduction 2", field: "MOD REDUCE 2", control: .range(min: -20, max: 0, step: 0.5, unit: "%")),
            SettingsItem("Multiplex Power Threshold", field: "ITU412 THR", control: .offRange(min: -2, max: 10, step: 0.1, unit: "dB", slider: true)),
        ]),
    ])

    private static let network = SettingsTab(title: "Network", symbolName: "network", sections: [
        SettingsSection(title: "Serial", items: [
            SettingsItem("Serial Interface Type", field: "INTERFACE TYPE", control: .segmented(options: [SettingsOption("Direct"), SettingsOption("Modem")])),
            SettingsItem("Modem Initialization String", field: "MODEM INIT STRING", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
        ]),
        SettingsSection(title: "Ethernet", items: [
            SettingsItem("IP Address", field: "NETWORK IP ADDRESS", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
            SettingsItem("Subnet Mask", field: "NETWORK SUBNET MASK", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
            SettingsItem("Gateway", field: "NETWORK GATEWAY", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
            SettingsItem("IP Port", field: "NETWORK PORT", control: .integer(min: 1, max: 65_535, unit: "")),
            SettingsItem("Terminal Port", field: "TERMINAL PORT", control: .integer(min: 1, max: 65_535, unit: "")),
        ]),
        SettingsSection(title: "SNMP", items: [
            SettingsItem("SNMP on boot-up", field: "START SNMP", control: .toggle(offLabel: "Disable", onLabel: "Enable", offAliases: ["Disable", "No"], onAliases: ["Enable", "Yes"])),
            SettingsItem("Primary Manager Address", field: "PRIM SNMP ADDRESS", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
            SettingsItem("Primary Manager Port", field: "PRIM SNMP PORT", control: .integer(min: 1, max: 65_535, unit: "")),
            SettingsItem("Secondary Manager Address", field: "SECOND SNMP ADDRESS", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
            SettingsItem("Secondary Manager Port", field: "SECOND SNMP PORT", control: .integer(min: 1, max: 65_535, unit: "")),
            SettingsItem("Community Read String", field: "SNMP READ", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
            SettingsItem("Community Write String", field: "SNMP WRITE", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
        ]),
    ], displaysAllSections: true)

    private static let stereoEncoder = SettingsTab(title: "Stereo Encoder", symbolName: "dot.radiowaves.left.and.right", sections: [
        SettingsSection(title: "Composite Outputs", items: [
            SettingsItem("Composite 1 Level", field: "COMP1 OUT", control: .range(min: -12.3, max: 12, step: 0.1, unit: "dBu"), note: "Only values exposed by the verified firmware profile can be submitted."),
            SettingsItem("Composite 2 Level", field: "COMP2 OUT", control: .range(min: -12.3, max: 12, step: 0.1, unit: "dBu"), note: "Only values exposed by the verified firmware profile can be submitted."),
        ]),
        SettingsSection(title: "Stereo Encoder", items: [
            SettingsItem("Modulation Mode", field: "MODULATION", control: .picker(options: [
                SettingsOption("Stereo"), SettingsOption("PilotOff", aliases: ["PilotOff", "pilotoff"]),
                SettingsOption("Mono-L", aliases: ["Mono-L", "mono-L"]),
                SettingsOption("Mono-R", aliases: ["Mono-R", "mono-R"]),
                SettingsOption("Mono-Sum", aliases: ["Mono-Sum", "mono-SUM"]), SettingsOption("SSB"),
            ], includesDevicePresets: false)),
            SettingsItem("19 kHz Pilot Reference", field: "PILOT REF", control: .segmented(options: [
                SettingsOption("0°", aliases: ["0°", "0 deg"]), SettingsOption("90°", aliases: ["90°", "90 deg"]),
                SettingsOption("180°", aliases: ["180°", "180 deg"]), SettingsOption("270°", aliases: ["270°", "270 deg"]),
            ])),
            SettingsItem("Diversity Delay", field: "DIVERSITY DELAY", control: .toggle(offLabel: "Out", onLabel: "In", offAliases: ["Out"], onAliases: ["In"])),
            SettingsItem("19 kHz Pilot Level", field: "PILOT LEVEL", control: .range(min: 6, max: 12, step: 0.1, unit: "%")),
        ]),
    ], displaysAllSections: true)

    private static let gpiFunctions: [SettingsOption] = [
        "Any Factory Preset", "Any User Preset", "Input: Analog", "Input: Digital", "Input: Digital+J.17",
        "Bypass", "Tone", "Exit Test", "Stereo", "SSB", "Mono From Left", "Mono From Right", "Mono From Sum",
        "MOD Reduction 1", "MOD Reduction 2", "Reset Clock To Hour", "Reset To Midnight", "Monitor Mute",
        "Analog Out Delay In", "Analog Out Delay Out", "Digital Out 1 Delay In", "Digital Out 1 Delay Out",
        "Digital Out 2 Delay In", "Digital Out 2 Delay Out", "FM Polarity Positive", "FM Polarity Negative", "No Function",
    ].map { SettingsOption($0) }

    private static let tallyOptions = [
        SettingsOption("Input: Analog"), SettingsOption("Input: Digital"), SettingsOption("Analog Input Silent"),
        SettingsOption("AES Input Silent"), SettingsOption("AES Input Error"),
        SettingsOption("Sync Reference Input Error", aliases: ["Sync Reference Input Error", "Sync Input Error"]),
        SettingsOption("No Function"),
    ]

    private static let remoteIO = SettingsTab(title: "Remote I/O", symbolName: "switch.2", sections: [
        SettingsSection(title: "GPI Inputs", items: (1...8).map { number in
            SettingsItem("Input #\(number)", field: "REMOTE CONTACT \(number)", control: .picker(options: gpiFunctions, includesDevicePresets: true), writePolicy: .typedText, note: "Factory and user preset names are loaded from the connected processor.")
        }),
        SettingsSection(title: "Tally Outputs", items: [1, 2].map { number in
            SettingsItem("Tally #\(number)", field: "TALLY \(number)", control: .picker(options: tallyOptions, includesDevicePresets: false))
        }),
        SettingsSection(title: "Silence Detection", items: [
            SettingsItem("Analog Fallback", field: "ANALOG FALLBACK", control: .toggle(offLabel: "No", onLabel: "Yes", offAliases: ["No"], onAliases: ["Yes"])),
            SettingsItem("Digital Fallback", field: "DIGITAL FALLBACK", control: .toggle(offLabel: "No", onLabel: "Yes", offAliases: ["No"], onAliases: ["Yes"])),
            SettingsItem("Silence Threshold", field: "SILENCE THR", control: .range(min: -60, max: -20, step: 1, unit: "dB")),
            SettingsItem("Silence Delay", field: "SILENCE DELAY", control: .range(min: 2, max: 60, step: 1, unit: "sec")),
        ]),
    ])

    private static let digitalRadio = SettingsTab(title: "HD / Digital Radio", symbolName: "hifispeaker.2", sections: [
        SettingsSection(title: "Processing", items: [
            SettingsItem("HF Shelf EQ", field: "HD EQ LOCATE", control: .segmented(options: [SettingsOption("Pre"), SettingsOption("Post")])),
            SettingsItem("Bandwidth", field: "HD BW", control: .picker(options: (15...20).map { SettingsOption("\($0) kHz", aliases: ["\($0) kHz", "\($0).0"]) }, includesDevicePresets: false)),
            SettingsItem("Stereo / Mono", field: "HD STEREO MONO", control: .segmented(options: ["Stereo", "Mono-L", "Mono-R", "Mono-Sum"].map { SettingsOption($0) })),
            SettingsItem("Polarity", field: "HD POLARITY", control: .segmented(options: [SettingsOption("Positive"), SettingsOption("Negative")])),
        ]),
        SettingsSection(title: "Loudness", items: [
            SettingsItem("1770 Loudness Control Threshold", field: "BS1770 LDNES CTRL THR", control: .range(min: -31, max: -11, step: 0.1, unit: "dB")),
            SettingsItem("1770 Safety Limiter", field: "BS1770 SAFETY LIMITER", control: .toggle(offLabel: "Off", onLabel: "On", offAliases: ["Off"], onAliases: ["On"])),
        ]),
        SettingsSection(title: "Diversity", items: [
            SettingsItem("Diversity Delay Trim", field: "DIVERSITY DELAY ADJ", control: .delayTrim(stepMicroseconds: 15.625), note: "Displayed in milliseconds; each step is exactly 15.625 µs."),
        ]),
    ], displaysAllSections: true)

    private static let rds = SettingsTab(title: "RDS", symbolName: "antenna.radiowaves.left.and.right", sections: [
        SettingsSection(title: "Program", items: [
            SettingsItem("Program Service (PS)", field: "RDS DYNAMIC PS", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
            SettingsItem("Dynamic Program Service Speed", field: "RDS DPS SPEED", control: .picker(options: (2...9).map { SettingsOption("\($0) sec", aliases: ["\($0) sec", "\($0)"]) }, includesDevicePresets: false)),
            SettingsItem("Dynamic Program Service Timeout", field: "RDS DPS TIMEOUT", control: .offRange(min: 1, max: 255, step: 1, unit: "min", slider: false)),
            SettingsItem("Radio Text", field: "RDS RADIO TEXT", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
            SettingsItem("Radio Text Speed", field: "RDS RADIO TEXT SPEED", control: .picker(options: ["Off", "5 sec", "10 sec", "15 sec", "20 sec", "25 sec", "30 sec", "35 sec", "40 sec", "45 sec"].map {
                SettingsOption($0, aliases: [$0, $0.replacingOccurrences(of: " sec", with: "")])
            }, includesDevicePresets: false)),
            SettingsItem("Program Identification (PI)", field: "RDS PROGRAM ID", control: .text(maxLength: nil, validation: .hexadecimal), writePolicy: .typedText),
            SettingsItem("Program Type (PTY)", field: "RDS PROGRAM TYPE", control: .integer(min: 0, max: 31, unit: ""), writePolicy: .typedText),
            SettingsItem("Program Type Name (PTYN)", field: "RDS PROGRAM NAME", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
            SettingsItem("Music / Speech", field: "RDS MUSIC SPEECH", control: .segmented(options: [SettingsOption("Speech"), SettingsOption("Music")])),
            SettingsItem("Decoder Info", field: "RDS DECODER INFO", control: .segmented(options: [SettingsOption("Mono"), SettingsOption("Stereo")])),
            SettingsItem("Traffic Program", field: "RDS TRAFFIC PROGRAM", control: .toggle(offLabel: "No", onLabel: "Yes", offAliases: ["No"], onAliases: ["Yes"])),
            SettingsItem("TA Timeout", field: "RDS TA TIMEOUT", control: .offRange(min: 1, max: 255, step: 1, unit: "min", slider: false)),
            SettingsItem("Current Time", field: "RDS TIME", control: .toggle(offLabel: "No", onLabel: "Yes", offAliases: ["No"], onAliases: ["Yes"])),
            SettingsItem("EAS Text", field: "RDS EAS", control: .text(maxLength: nil, validation: .plain), writePolicy: .typedText),
        ]),
        SettingsSection(title: "RDS Modulator", items: [
            SettingsItem("57 kHz RDS Subcarrier", field: "RDS ACTIVE", control: .toggle(offLabel: "Disable", onLabel: "Enable", offAliases: ["Disable", "No"], onAliases: ["Enable", "Yes"])),
            SettingsItem("Subcarrier Level", field: "RDS LEVEL", control: .range(min: 0, max: 12, step: 0.1, unit: "%")),
        ]),
        SettingsSection(title: "RDS Terminal Server", items: [
            SettingsItem("Terminal Echo", field: "RDS ECHO", control: .toggle(offLabel: "Disable", onLabel: "Enable", offAliases: ["Disable", "No"], onAliases: ["Enable", "Yes"])),
            SettingsItem("Terminal Header", field: "RDS HEADER", control: .toggle(offLabel: "Disable", onLabel: "Enable", offAliases: ["Disable", "No"], onAliases: ["Enable", "Yes"])),
            SettingsItem("IP Port", field: "RDS PORT", control: .integer(min: 1, max: 65_535, unit: "")),
            SettingsItem("Source IP Address", field: "RDS SOURCE IP", control: .text(maxLength: nil, validation: .ipAddress), writePolicy: .typedText),
        ]),
    ])

    private static let rdsAF = SettingsTab(title: "RDS AF", symbolName: "list.number", sections: [
        SettingsSection(title: "Alternate Frequencies 1–12", items: (1...12).map { number in
            SettingsItem("AF \(number)", field: "RDS ALTERNATE FREQUENCY \(number)", control: .alternateFrequency)
        }),
        SettingsSection(title: "Alternate Frequencies 13–24", items: (13...24).map { number in
            SettingsItem("AF \(number)", field: "RDS ALTERNATE FREQUENCY \(number)", control: .alternateFrequency)
        }),
    ])
}
