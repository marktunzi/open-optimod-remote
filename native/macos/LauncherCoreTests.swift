import Foundation

@main
struct LauncherCoreTests {
    static func main() throws {
        let endpoint = BackendHealth.endpoint
        precondition(endpoint.absoluteString == "http://127.0.0.1:5701/api/health")
        precondition(BackendHealth.servicePort(nil) == 5701)
        precondition(BackendHealth.servicePort("15701") == 15701)
        precondition(BackendHealth.servicePort("80") == 5701)
        precondition(BackendHealth.servicePort("x") == 5701)

        let valid = Data("open-optimod-remote/0.2.0".utf8)
        precondition(BackendHealth.isOwnService(statusCode: 200, data: valid))
        precondition(!BackendHealth.isOwnService(statusCode: 503, data: valid))
        precondition(!BackendHealth.isOwnService(statusCode: 200, data: Data("other-service".utf8)))
        precondition(!BackendHealth.isOwnService(statusCode: 200, data: nil))

        print("LauncherCoreTests passed")
    }
}
