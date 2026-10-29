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
        // The 5500i client restores setup, automation and network files as files.
        let sorted55 = try PresetBackup.sort([
            PresetBackupFile(name: "Setup.orbsetup", document: "s"),
            PresetBackupFile(name: "Automation.orb55autom", document: "a"),
            PresetBackupFile(name: "OOR TEST.ORB55USER", document: "p"),
        ], adapterID: "pc-remote-5500i-3.1.0.2")
        precondition(sorted55.setup == nil && sorted55.presets.count == 3)
        // The 5500 client restores Setup.orb55setup and its automation file as files; it has no network file.
        let sorted5500 = try PresetBackup.sort([
            PresetBackupFile(name: "Setup.orb55setup", document: "s"),
            PresetBackupFile(name: "Automation.orb55autom", document: "a"),
            PresetBackupFile(name: "ROCK.orb55user", document: "p"),
        ], adapterID: "pc-remote-5500-1.2.8.24")
        precondition(sorted5500.setup == nil && sorted5500.presets.count == 3)
        precondition((try? PresetBackup.sort([PresetBackupFile(name: "Network.orb55net", document: "n")], adapterID: "pc-remote-5500-1.2.8.24")) == nil)
        precondition(PresetFileFormat.backsUpFiles("pc-remote-5500-1.2.8.24") && PresetFileFormat.replacesOnAir("pc-remote-5500-1.2.8.24"))
        // The 8700HD client's Restore is the 5500's with .orb86 names.
        let sorted8700 = try PresetBackup.sort([
            PresetBackupFile(name: "Setup.orb86setup", document: "s"),
            PresetBackupFile(name: "Automation.orb86autom", document: "a"),
            PresetBackupFile(name: "ROCK.ORB86USER", document: "p"),
        ], adapterID: "pc-remote-8700hd-1.0.2.161")
        precondition(sorted8700.setup == nil && sorted8700.presets.count == 3)
        precondition((try? PresetBackup.sort([PresetBackupFile(name: "ROCK.orb55user", document: "p")], adapterID: "pc-remote-8700hd-1.0.2.161")) == nil)
        precondition(PresetFileFormat.backsUpFiles("pc-remote-8700hd-1.0.2.161"))
        // The 2300 and 8400 clients back up and restore user preset files; Save As only, as before.
        for id in ["pc-remote-2300-2.0.3.85", "pc-remote-8400-3.0.5.7"] {
            precondition(PresetFileFormat.backsUpFiles(id) && !PresetFileFormat.replacesOnAir(id))
        }
        let sorted2300 = try PresetBackup.sort([PresetBackupFile(name: "ROCK.orb23user", document: "p")], adapterID: "pc-remote-2300-2.0.3.85")
        precondition(sorted2300.setup == nil && sorted2300.presets.count == 1)
        let sorted8400 = try PresetBackup.sort([PresetBackupFile(name: "EURO AC .orb", document: "p")], adapterID: "pc-remote-8400-3.0.5.7")
        precondition(sorted8400.presets.count == 1 && PresetFileFormat.forAdapter("pc-remote-8400-3.0.5.7").model == "OPTIMOD-FM 8400")
        // The 6300 client's Restore is the 8700HD's; the 8700i's the 5500i's (with a network file).
        let sorted6300 = try PresetBackup.sort([
            PresetBackupFile(name: "Setup.orb63setup", document: "s"),
            PresetBackupFile(name: "Automation.orb63autom", document: "a"),
            PresetBackupFile(name: "ROCK.orb63user", document: "p"),
        ], adapterID: "pc-remote-6300-4.1.1.19")
        precondition(sorted6300.setup == nil && sorted6300.presets.count == 3)
        let sorted8700i = try PresetBackup.sort([
            PresetBackupFile(name: "Setup.orb87setup", document: "s"),
            PresetBackupFile(name: "Network.orb87net", document: "n"),
            PresetBackupFile(name: "ROCK.orb87user", document: "p"),
        ], adapterID: "pc-remote-8700i-1.5.1.11")
        precondition(sorted8700i.setup == nil && sorted8700i.presets.count == 3)
        precondition(PresetFileFormat.forAdapter("pc-remote-6300-4.1.1.19").maxNameLength == 18)
        precondition(PresetFileFormat.forAdapter("pc-remote-8700i-1.5.1.11").maxNameLength == 20)
        precondition(PresetBackup.crc32(Data("123456789".utf8)) == 0xcbf4_3926)
        // A name from the processor never leaves the backup folder.
        precondition(PresetBackup.entryName("../../evil.orb") == ".._.._evil.orb" && PresetBackup.entryName("..") == "_")
        precondition(PresetBackup.entryName("a\\b.orb") == "a_b.orb" && PresetBackup.entryName("EURO AC .orb") == "EURO AC .orb")
        // Names that would land on the same file once unzipped stay apart.
        precondition(PresetBackup.uniqueEntryNames(["a/b.orb", "a_b.orb", "A_B.ORB", "..", ".", "ROCK", "rock"])
            == ["a_b.orb", "a_b 2.orb", "A_B 3.ORB", "_", "_ 2", "ROCK", "rock 2"])
        precondition(PresetBackup.uniqueEntryNames(["Caf\u{e9}.orb", "Cafe\u{301}.orb", ".orb", ".orb"]) == ["Caf\u{e9}.orb", "Cafe\u{301} 2.orb", ".orb", ".orb 2"])
        let twice = PresetBackup.zip(folder: "f", files: [("x.orb", Data()), ("X.orb", Data())])
        precondition(twice.range(of: Data("f/X 2.orb".utf8)) != nil)
        let zip = PresetBackup.zip(folder: "f", files: [("a.txt", Data("hi".utf8))])
        precondition(zip.prefix(4) == Data([0x50, 0x4b, 0x03, 0x04]) && zip.count == 30 + 7 + 2 + 46 + 7 + 22)
        // DOS date of 28 October 2026: (46 << 9) | (10 << 5) | 28.
        let dated = PresetBackup.zip(folder: "f", files: [("a", Data())],
                                     date: Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: 28))!)
        precondition(dated[12] == UInt8((46 << 9 | 10 << 5 | 28) & 0xff) && dated[13] == UInt8((46 << 9 | 10 << 5 | 28) >> 8))
        precondition(PresetBackup.safeFilename("192.168.1.177") == "192.168.1.177")

        precondition(PresetFileFormat.forAdapter("pc-remote-5700i-3.0.1.20").fileExtension == "orb57user")
        precondition(PresetFileFormat.forAdapter("pc-remote-5500-1.2.8.24").fileExtension == "orb55user")
        precondition(PresetFileFormat.forAdapter("pc-remote-8700hd-1.0.2.161").fileExtension == "orb86user")
        precondition(PresetFileFormat.forAdapter("pc-remote-5700i-family").fileExtension == "orb57user")
        precondition(PresetFileFormat.forAdapter("pc-remote-5500i-3.1.0.2").fileExtension == "orb55user")
        precondition(PresetFileFormat.backsUpFiles("pc-remote-5500i-3.1.0.2") && PresetFileFormat.replacesOnAir("pc-remote-5500i-3.1.0.2"))
        precondition(PresetFileFormat.backsUpFiles("pc-remote-8700i-1.5.1.11") && PresetFileFormat.backsUpFiles("pc-remote-8400-3.0.5.7"))
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
