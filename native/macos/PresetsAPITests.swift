import Foundation

@main
struct PresetsAPITests {
    static func main() throws {
        let factory = DevicePresetPayload(name: "GREGG OPEN", kind: "Factory")
        let modified = DevicePresetPayload(name: "modif GREGG OPEN", kind: "Unsaved")
        precondition(PresetBrowserFilter.factory.includes(factory))
        precondition(!PresetBrowserFilter.factory.includes(modified))
        // The modified on-air preset is shown as on air, not as a preset.
        precondition(!PresetBrowserFilter.all.includes(modified))
        precondition(PresetBrowserFilter.all.includes(factory))

        let snapshot = try JSONDecoder().decode(DeviceSnapshotPayload.self, from: Data("""
        {"connected":true,"firmware":"3.0.1.20","host":"192.0.2.115","system":null,
         "processing":{"name":"modif OOR UI 1","fields":{}},"error":null,"write_enabled":true,"session_id":"s",
         "presets":[{"name":"GREGG OPEN","kind":"Factory"},{"name":"OOR UI 1","kind":"User"},{"name":"OTHER","kind":"User"},
                    {"name":"modif OOR UI 1","kind":"Unsaved"}],
         "adapter_id":"pc-remote-5700i-3.0.1.20","device_id":"d","revision":3,
         "capabilities":{"parameter_reads":true,"parameter_writes":true,"live_meters":true,"preset_catalog":true,
                         "preset_store":true,"preset_delete":true}}
        """.utf8))
        let context = PresetBrowserContext(snapshot: snapshot)
        precondition(context.replaceName == "OOR UI 1", "5700i Save replaces the on-air user preset")
        precondition(context.canSaveAs)
        precondition(!context.canDelete("OOR UI 1"), "the modified on-air preset cannot be deleted")
        precondition(context.canDelete("OTHER"))
        precondition(!context.canDelete("GREGG OPEN"), "factory presets cannot be deleted")
        precondition(!context.canRename("OOR UI 1"), "rename needs the preset on air without changes")
        precondition(context.nameProblem("OOR UI 1") == "A preset with this name already exists.")
        precondition(context.nameProblem("GREGG OPEN") == "A factory preset has this name.")
        precondition(context.nameProblem("modif X") != nil)
        precondition(context.nameProblem("A[1]") != nil)
        precondition(context.nameProblem(String(repeating: "A", count: 21)) == "Use at most 20 characters.")
        precondition(context.nameProblem("NEW ONE") == nil)

        let sorted = try PresetBackup.sort([
            PresetBackupFile(name: "Setup.orb57setup", document: "s"),
            PresetBackupFile(name: "OOR UI 1.orb57user", document: "p"),
        ], adapterID: "pc-remote-5700i-3.0.1.20")
        precondition(sorted.setup?.name == "Setup.orb57setup" && sorted.presets.count == 1)
        precondition((try? PresetBackup.sort([PresetBackupFile(name: "x.txt", document: "")], adapterID: "pc-remote-5700i-3.0.1.20")) == nil)
        precondition(PresetBackup.crc32(Data("123456789".utf8)) == 0xcbf4_3926)
        let zip = PresetBackup.zip(folder: "f", files: [("a.txt", Data("hi".utf8))])
        precondition(zip.prefix(4) == Data([0x50, 0x4b, 0x03, 0x04]) && zip.count == 30 + 7 + 2 + 46 + 7 + 22)
        precondition(PresetBackup.safeFilename("192.168.1.177") == "192.168.1.177")

        precondition(PresetFileFormat.forAdapter("pc-remote-5700i-3.0.1.20").fileExtension == "orb57user")
        precondition(PresetFileFormat.forAdapter("pc-remote-5500-1.2.8.24").fileExtension == "orb55user")
        precondition(PresetFileFormat.forAdapter("pc-remote-8700hd-1.0.2.161").fileExtension == "orb86user")
        precondition(PresetFileFormat.forAdapter("pc-remote-5700i-family").fileExtension == "orb57user")
        precondition(PresetFileFormat.forAdapter("pc-remote-5500i-family").fileExtension == "orb")
        precondition(PresetFileFormat.forAdapter(nil).model == "OPTIMOD")

        let request = PresetRecallRequest(
            name: "GREGG OPEN",
            expectedName: "NEWS-TALK",
            confirmed: true,
            expectedHost: "192.0.2.115",
            expectedSession: "session"
        )
        let json = try JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as! [String: Any]
        precondition(json["expected_name"] as? String == "NEWS-TALK")
        precondition(json["confirmed"] as? Bool == true)

        let apply = PresetApplyRequest(
            document: "OptimodVersion=<5700.51>\r\nEnd Preset<end>\r\n",
            expectedName: "GREGG OPEN",
            confirmed: true,
            expectedHost: "192.0.2.115",
            expectedSession: "session"
        )
        let applyJSON = try JSONSerialization.jsonObject(with: JSONEncoder().encode(apply)) as! [String: Any]
        precondition(applyJSON["document"] as? String == apply.document)
        precondition(applyJSON["expected_name"] as? String == "GREGG OPEN")
        print("PresetsAPITests passed")
    }
}
