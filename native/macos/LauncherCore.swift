import Foundation

enum BackendHealth {
    static let origin = "http://127.0.0.1:5701"
    static let endpoint = URL(string: "\(origin)/api/health")!
    static let expectedPrefix = "open-optimod-remote/"

    static func isOwnService(statusCode: Int, data: Data?) -> Bool {
        guard statusCode == 200,
              let data,
              let body = String(data: data, encoding: .utf8)
        else {
            return false
        }
        return body.hasPrefix(expectedPrefix)
    }
}
