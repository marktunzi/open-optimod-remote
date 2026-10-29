import Foundation

/// Test and development settings: the environment, else this app's own
/// preferences. A test build has its own bundle identifier, so settings
/// written to its domain survive Sparkle's relaunch and never reach the
/// installed app's domain.
enum LaunchOverride {
    static let keys = ["OPEN_OPTIMOD_PORT", "OPTIMOD_DATA_DIR", "OPEN_OPTIMOD_WINDOW_CONTROL", "OPEN_OPTIMOD_UPDATE_FEED"]

    static func value(_ key: String, environment: [String: String] = ProcessInfo.processInfo.environment,
                      defaults: UserDefaults = .standard) -> String? {
        environment[key] ?? defaults.string(forKey: key)
    }
}

enum BackendHealth {
    /// `OPEN_OPTIMOD_PORT` moves a development or test instance next to an
    /// installed app; the service reads the same variable.
    static let port = servicePort(LaunchOverride.value("OPEN_OPTIMOD_PORT"))
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
