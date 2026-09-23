import Foundation

@main
struct ConnectionsAPITests {
    static func main() throws {
        let data = Data(#"{"devices":[{"id":"one","name":"Studio","host":"192.0.2.115","port":6201,"terminal_port":23,"has_code":true}],"credential_storage":"local"}"#.utf8)
        let response = try JSONDecoder().decode(ConnectionsResponse.self, from: data)
        precondition(response.devices.count == 1)
        precondition(response.devices[0].terminalPort == 23)
        precondition(response.devices[0].hasCode)
        precondition(response.devices[0].processorModel == .auto)

        let edit = ConnectionEdit(id: "one", name: "Studio", host: "192.0.2.115", port: 6201, terminalPort: 23)
        let encoded = String(data: try JSONEncoder().encode(edit), encoding: .utf8)!
        precondition(encoded.contains(#""terminal_port":23"#))
        precondition(encoded.contains(#""model":"auto""#))
        precondition(!encoded.localizedCaseInsensitiveContains("code"))

        let devices = [
            SavedConnection(id: "a", name: "A", host: "192.0.2.1", port: 6201, terminalPort: 23, hasCode: true),
            SavedConnection(id: "b", name: "B", host: "192.0.2.2", port: 6201, terminalPort: 23, hasCode: false),
            SavedConnection(id: "c", name: "C", host: "192.0.2.3", port: 6201, terminalPort: 23, hasCode: true),
        ]
        precondition(ConnectionSelection.next(afterDeleting: "b", from: devices) == "c")
        precondition(ConnectionSelection.next(afterDeleting: "c", from: devices) == "b")
        precondition(ConnectionSelection.next(afterDeleting: "a", from: [devices[0]]) == nil)
        precondition(ConnectionSelection.mustDisconnect(connected: true, activeID: "a", deletingID: "a"))
        precondition(!ConnectionSelection.mustDisconnect(connected: true, activeID: "a", deletingID: "b"))

        print("ConnectionsAPITests passed")
    }
}
