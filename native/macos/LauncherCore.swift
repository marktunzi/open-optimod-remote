import Foundation

enum BackendHealth {
    /// `OPEN_OPTIMOD_PORT` moves a development or test instance next to an
    /// installed app; the service reads the same variable.
    static let port = servicePort(ProcessInfo.processInfo.environment["OPEN_OPTIMOD_PORT"])
    static let origin = "http://127.0.0.1:\(port)"
    static let endpoint = URL(string: "\(origin)/api/health")!
    static let expectedPrefix = "open-optimod-remote/"

    /// A service path, in the processor's own session when `device` is set
    /// (`/api/d/<device>/<path>`), so a window never reaches another one.
    static func api(_ path: String, device: String? = nil) -> URL {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_.:[]"))
        guard let device, let key = device.addingPercentEncoding(withAllowedCharacters: allowed) else {
            return URL(string: "\(origin)/api/\(path)")!
        }
        return URL(string: "\(origin)/api/d/\(key)/\(path)")!
    }

    static func servicePort(_ value: String?) -> Int {
        guard let value, let port = Int(value), (1024...65535).contains(port) else { return 5701 }
        return port
    }

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
