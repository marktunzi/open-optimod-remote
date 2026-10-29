import Foundation

/// What an update may do while processors are connected. An update quits and
/// relaunches the app, and quitting ends every processor session, so it only
/// installs once the service has no session left: the user disconnects
/// explicitly, or the update waits.
enum UpdateGate {
    /// One processor whose session an update would end.
    struct Processor: Equatable {
        let model: String
        let host: String

        var label: String {
            let name = model.replacingOccurrences(of: "optimod-", with: "").uppercased()
            return name.isEmpty ? host : "\(name) at \(host)"
        }
    }

    /// The processors in the service's `/api/sessions` answer that are
    /// connected or waiting to reconnect. nil = the answer was not readable,
    /// which counts as "not known to be idle".
    static func activeProcessors(sessionsJSON data: Data?) -> [Processor]? {
        guard let data,
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let sessions = object["sessions"] as? [[String: Any]]
        else { return nil }
        return sessions.compactMap { session in
            let connected = session["connected"] as? Bool ?? false
            let reconnecting = session["reconnect"].map { !($0 is NSNull) } ?? false
            guard connected || reconnecting else { return nil }
            return Processor(model: session["model"] as? String ?? "", host: session["host"] as? String ?? "")
        }
    }

    /// The question asked before an update ends these sessions.
    static func message(version: String, processors: [Processor]) -> (title: String, detail: String) {
        let count = processors.count
        let title = count == 1
            ? "Disconnect the processor to install \(version)?"
            : "Disconnect \(count) processors to install \(version)?"
        let names = processors.map { "• \($0.label)" }.joined(separator: "\n")
        let detail = """
        Installing restarts Open Optimod Remote, which ends \(count == 1 ? "this session" : "these sessions"):

        \(names)

        Nothing on the processor changes. Choose Later to keep working; the update stays ready and installs when you quit or choose Install Update… in the app menu.
        """
        return (title, detail)
    }

    /// The feed the app reads. A test build may name another one with
    /// OPEN_OPTIMOD_UPDATE_FEED; every update is still checked against the
    /// EdDSA key in Info.plist, so another feed cannot install anything unsigned.
    static func feedOverride(_ environment: [String: String] = ProcessInfo.processInfo.environment) -> URL? {
        guard let value = environment["OPEN_OPTIMOD_UPDATE_FEED"], let url = URL(string: value),
              ["https", "http", "file"].contains(url.scheme ?? "")
        else { return nil }
        return url
    }
}
