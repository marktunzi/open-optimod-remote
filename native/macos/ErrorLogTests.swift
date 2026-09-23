import Foundation

@main
struct ErrorLogTests {
    static func main() {
        let message = "Could not reach 192.168.50.20; retry 10.0.0.2 and 172.31.4.9."
        precondition(ErrorLogStore.redact(message) == "Could not reach [local Optimod]; retry [local Optimod] and [local Optimod].")
        precondition(ErrorLogStore.redact("Preset readback failed") == "Preset readback failed")
        print("ErrorLogTests passed")
    }
}
