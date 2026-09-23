import Foundation

@main
struct LauncherCoreTests {
    static func main() throws {
        let endpoint = BackendHealth.endpoint
        precondition(endpoint.absoluteString == "http://127.0.0.1:5701/api/health")

        let valid = Data("open-optimod-remote/0.1.0-alpha.1".utf8)
        precondition(BackendHealth.isOwnService(statusCode: 200, data: valid))
        precondition(!BackendHealth.isOwnService(statusCode: 503, data: valid))
        precondition(!BackendHealth.isOwnService(statusCode: 200, data: Data("other-service".utf8)))
        precondition(!BackendHealth.isOwnService(statusCode: 200, data: nil))

        print("LauncherCoreTests passed")
    }
}
