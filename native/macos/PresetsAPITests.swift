import Foundation

@main
struct PresetsAPITests {
    static func main() throws {
        let factory = DevicePresetPayload(name: "GREGG OPEN", kind: "Factory")
        let modified = DevicePresetPayload(name: "modif GREGG OPEN", kind: "Unsaved")
        precondition(PresetBrowserFilter.factory.includes(factory))
        precondition(!PresetBrowserFilter.factory.includes(modified))
        precondition(PresetBrowserFilter.modified.includes(modified))

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
