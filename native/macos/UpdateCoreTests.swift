import Foundation

@main
struct UpdateCoreTests {
    static func main() throws {
        let sessions = Data("""
        {"sessions":[
          {"model":"optimod-5700i","host":"192.168.1.177","connected":true,"reconnect":null},
          {"model":"optimod-8400","host":"192.168.1.176","connected":false,"reconnect":{"attempt":2}},
          {"model":"optimod-6300","host":"192.168.1.188","connected":false,"reconnect":null}
        ]}
        """.utf8)
        let active = UpdateGate.activeProcessors(sessionsJSON: sessions)
        precondition(active == [
            UpdateGate.Processor(model: "optimod-5700i", host: "192.168.1.177"),
            UpdateGate.Processor(model: "optimod-8400", host: "192.168.1.176"),
        ])
        precondition(active?.first?.label == "5700I at 192.168.1.177")
        precondition(UpdateGate.activeProcessors(sessionsJSON: Data(#"{"sessions":[]}"#.utf8)) == [])
        // An unreadable answer is not "idle".
        precondition(UpdateGate.activeProcessors(sessionsJSON: Data("<html>".utf8)) == nil)
        precondition(UpdateGate.activeProcessors(sessionsJSON: nil) == nil)

        let one = UpdateGate.message(version: "0.3.1", processors: Array(active!.prefix(1)))
        precondition(one.title == "Disconnect the processor to install 0.3.1?")
        precondition(one.detail.contains("• 5700I at 192.168.1.177"))
        let two = UpdateGate.message(version: "0.3.1", processors: active!)
        precondition(two.title == "Disconnect 2 processors to install 0.3.1?")
        precondition(two.detail.contains("these sessions"))

        precondition(UpdateGate.feedOverride([:]) == nil)
        precondition(UpdateGate.feedOverride(["OPEN_OPTIMOD_UPDATE_FEED": "http://127.0.0.1:8765/appcast.xml"])?.port == 8765)
        precondition(UpdateGate.feedOverride(["OPEN_OPTIMOD_UPDATE_FEED": "javascript:alert(1)"]) == nil)

        print("UpdateCoreTests passed")
    }
}
