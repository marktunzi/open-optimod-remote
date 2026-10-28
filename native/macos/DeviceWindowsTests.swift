import Foundation

@main
struct DeviceWindowsTests {
    static func main() throws {
        // A processor is its saved connection, else its address; never a port.
        precondition(DeviceRef.identity(deviceID: "d1", host: "192.168.1.171") == "d1")
        precondition(DeviceRef.identity(deviceID: nil, host: "192.168.1.171") == "host:192.168.1.171")
        precondition(DeviceRef.identity(deviceID: "", host: "") == nil)

        let state = Data("""
        {"connected":true,"host":"192.168.1.171","firmware":"8700HD 1.0.2.161","device_id":"d1",
         "session_id":"s1","adapter_id":"pc-remote-8700hd-1.0.2.161","model":"optimod-8700hd",
         "capabilities":{"connect":true,"parameter_reads":true,"parameter_writes":true,"live_meters":true,
         "preset_catalog":true,"preset_recall":true},"processing":{"name":"x","fields":{}},"revision":4}
        """.utf8)
        let session = try JSONDecoder().decode(DeviceSessionPayload.self, from: state)
        precondition(session.deviceID == "d1" && session.capabilities?.liveMeters == true)

        func device(_ adapter: String?, _ model: ProcessorModel?) -> DeviceRef {
            DeviceRef(id: "d1", name: "Studio", model: model, host: "192.168.1.171", firmware: "",
                      adapterID: adapter, capabilities: nil)
        }
        let hd = device("pc-remote-8700hd-1.0.2.161", .optimod8700HD)
        precondition(hd.subtitle == "Studio · OPTIMOD-FM 8700HD · 192.168.1.171")
        // Only the 5700i uses the native 5700i worksheet; no other model borrows it.
        // The others get native tabs from their own profile.
        precondition(ProfileSystemSettings.group(of: "RDS PROGRAM NAME") == "RDS")
        precondition(ProfileSystemSettings.section(of: "AI REF LEVEL", in: ProfileSystemSettings.group(of: "AI REF LEVEL")) == "Analog Input")
        precondition(ProfileSystemSettings.label("AI REF LEVEL") == "Analog Input Reference Level")
        let text = DeviceFieldPayload(index: 0, value: .text("STATION"))
        let address = DeviceFieldPayload(index: 0, value: .text("192.168.1.10"))
        if case .unavailable = ProfileSystemSettings.item(for: "ACTUAL A OR D", label: nil, field: text, definition: nil)?.writePolicy {} else { preconditionFailure() }
        // A choice the profile cannot name is not shown; a text value is typed and read back.
        precondition(ProfileSystemSettings.item(for: "SOME FIELD", label: nil,
                                                field: DeviceFieldPayload(index: 3, value: DeviceValuePayload(type: "Int", value: .integer(3))),
                                                definition: nil) == nil)
        precondition(ProfileSystemSettings.item(for: "STATION ID", label: nil, field: text, definition: nil)?.writePolicy == .typedText)
        precondition(ProfileSystemSettings.item(for: "NETWORK IP", label: nil, field: address, definition: nil)?.control
                     == .text(maxLength: 255, validation: .ipAddress))
        // The client's own label wins, without its trailing colon.
        precondition(ProfileSystemSettings.item(for: "RDS PROGRAM NAME", label: "Program Service Name(PS):", field: text, definition: nil)?.label
                     == "Program Service Name(PS)")
        // Client pages read column by column; named group boxes become sections.
        let page = SystemLayoutPage(title: "Input", controls: [
            SystemLayoutControl(kind: "slider", name: "B", label: "B", x: 182, y: 16, options: []),
            SystemLayoutControl(kind: "slider", name: "A", label: "A", x: 13, y: 46, options: []),
        ], groups: [SystemLayoutGroup(x: 170, y: 0, w: 150, h: 100, text: " DIGITAL")])
        let fields = ["A": text, "B": text]
        let snapshot = try! JSONDecoder().decode(DeviceSnapshotPayload.self, from: Data("""
        {"connected":true,"firmware":"x","host":"h","system":{"name":"s","fields":{"A":{"index":0,"value":{"type":"Text","value":"1"}},"B":{"index":0,"value":{"type":"Text","value":"2"}}}},"processing":null,"error":null,"write_enabled":true,"session_id":"s","presets":[],"adapter_id":"a"}
        """.utf8))
        let context = SystemSettingsContext(snapshot: snapshot, definitions: [:], pages: [page])
        precondition(ProfileSystemSettings.clientSections(page, fields: fields, context: context).map(\.title) == ["Input", "Digital"])
        precondition(ProfileSystemSettings.tabs(context: context).first?.title == "Input")
        precondition(device("pc-remote-5700i-3.0.1.20", .optimod5700i).usesNative5700iSettings)
        for adapter in ["pc-remote-8700hd-1.0.2.161", "pc-remote-5500-1.2.8.24", "pc-remote-5500i-3.1.0.2", nil] {
            precondition(!device(adapter, nil).usesNative5700iSettings)
        }
        precondition(device(nil, .auto).modelLabel == "Optimod")

        // Window positions are kept per processor and kind.
        precondition(DeviceWebWindowController.autosaveName(kind: .meters, deviceID: "d1") == "OpenOptimod.meters.d1")
        precondition(DeviceWebWindowController.autosaveName(kind: .processing, deviceID: nil) == "OpenOptimod.processing.none")
        precondition(DeviceWindowKind.processing.defaultSize == NSSize(width: 1133, height: 500))
        // The Figma Meters frame; a narrower window scrolls the meters.
        precondition(DeviceWindowKind.meters.defaultSize == NSSize(width: 1133, height: 370))
        precondition(DeviceWindowKind.meters.minimumSize.width < 1133)
        // The page reports its meters' real size; the bridge accepts it.
        precondition(DeviceWindowBridge.messages.contains("meterSize"))
        precondition(DeviceWindowKind.startup == [.meters, .processing])
        print("DeviceWindowsTests passed")
    }
}
