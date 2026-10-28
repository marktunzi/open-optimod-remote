import Foundation

enum ProcessorModel: String, Codable, CaseIterable, Sendable {
    case auto
    case optimod5700i = "optimod-5700i"
    case optimod5500i = "optimod-5500i"
    case optimod5500 = "optimod-5500"
    case optimod5700FM = "optimod-5700-fm"
    case optimod5700HD = "optimod-5700-hd"
    case optimod8500 = "optimod-8500"
    case optimod2300 = "optimod-2300"
    case optimod6300 = "optimod-6300"
    case optimod8400 = "optimod-8400"
    case optimod8600 = "optimod-8600"
    case optimod8700i = "optimod-8700i"
    case optimod8700HD = "optimod-8700hd"
    case optimod9300 = "optimod-9300"
    case optimod9400 = "optimod-9400"

    /// The 8400 speaks PC Remote on TCP 51200, every other model on 6201.
    var defaultPort: Int { self == .optimod8400 ? 51200 : 6201 }
    static let defaultPorts: Set<Int> = [6201, 51200]

    var label: String {
        switch self {
        case .auto: "Auto Detect"
        case .optimod5700i: "OPTIMOD 5700i"
        case .optimod5500i: "OPTIMOD 5500i"
        case .optimod5500: "OPTIMOD 5500"
        case .optimod5700FM: "OPTIMOD 5700 FM"
        case .optimod5700HD: "OPTIMOD 5700 HD"
        case .optimod8500: "OPTIMOD 8500"
        case .optimod2300: "OPTIMOD 2300"
        case .optimod6300: "OPTIMOD 6300"
        case .optimod8400: "OPTIMOD-FM 8400"
        case .optimod8600: "OPTIMOD 8600"
        case .optimod8700i: "OPTIMOD 8700i"
        case .optimod8700HD: "OPTIMOD-FM 8700HD"
        case .optimod9300: "OPTIMOD 9300"
        case .optimod9400: "OPTIMOD 9400"
        }
    }
}

struct SavedConnection: Codable, Equatable, Sendable, Identifiable {
    let id: String
    let name: String
    let host: String
    let port: Int
    let terminalPort: Int
    let hasCode: Bool
    let model: ProcessorModel?
    /// The connection-list group, as in the original client.
    var group: String? = nil

    var processorModel: ProcessorModel { model ?? .auto }

    init(id: String, name: String, host: String, port: Int, terminalPort: Int, hasCode: Bool, model: ProcessorModel = .auto, group: String? = nil) {
        self.id = id
        self.name = name
        self.host = host
        self.port = port
        self.terminalPort = terminalPort
        self.hasCode = hasCode
        self.model = model
        self.group = group
    }

    enum CodingKeys: String, CodingKey {
        case id, name, host, port, model, group
        case terminalPort = "terminal_port"
        case hasCode = "has_code"
    }
}

struct ConnectionsResponse: Codable, Sendable {
    let devices: [SavedConnection]
    let groups: [String]?
}

struct ConnectionEdit: Codable, Sendable {
    let id: String?
    let name: String
    let host: String
    let port: Int
    let terminalPort: Int
    let model: ProcessorModel
    let group: String?

    init(id: String?, name: String, host: String, port: Int, terminalPort: Int, model: ProcessorModel = .auto, group: String? = nil) {
        self.id = id
        self.name = name
        self.host = host
        self.port = port
        self.terminalPort = terminalPort
        self.model = model
        self.group = group
    }

    enum CodingKeys: String, CodingKey {
        case id, name, host, port, model, group
        case terminalPort = "terminal_port"
    }
}

struct ConnectionsContext: Sendable, Equatable {
    let devices: [SavedConnection]
    /// The saved connections whose own session is connected; every processor
    /// has its own, so any number can be connected at once.
    var connectedIDs: Set<String> = []
    var groups: [String] = []

    func isConnected(_ id: String) -> Bool { connectedIDs.contains(id) }
}

/// New, Rename and Delete Group of the connection list.
enum ConnectionGroupEdit: Encodable {
    case create(String)
    case rename(String, to: String)
    case delete(String)

    private enum Keys: String, CodingKey { case action, name, newName = "new_name" }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .create(name):
            try container.encode("create", forKey: .action); try container.encode(name, forKey: .name)
        case let .rename(name, newName):
            try container.encode("rename", forKey: .action); try container.encode(name, forKey: .name)
            try container.encode(newName, forKey: .newName)
        case let .delete(name):
            try container.encode("delete", forKey: .action); try container.encode(name, forKey: .name)
        }
    }
}

enum ConnectionSelection {
    static func next(afterDeleting deletedID: String, from devices: [SavedConnection]) -> String? {
        guard let index = devices.firstIndex(where: { $0.id == deletedID }) else {
            return devices.first?.id
        }
        if devices.indices.contains(index + 1) { return devices[index + 1].id }
        if index > 0 { return devices[index - 1].id }
        return nil
    }

    static func mustDisconnect(connectedIDs: Set<String>, deletingID: String) -> Bool {
        connectedIDs.contains(deletingID)
    }

    /// The service keeps a saved code across name and port edits only; a new
    /// address or processor model may be another device.
    static func savedCodeSurvivesEdit(
        hasCode: Bool, savedHost: String, savedModel: ProcessorModel, host: String, model: ProcessorModel
    ) -> Bool {
        hasCode
            && savedHost.trimmingCharacters(in: .whitespacesAndNewlines) == host.trimmingCharacters(in: .whitespacesAndNewlines)
            && savedModel == model
    }
}

enum ConnectionsAPIError: LocalizedError, Sendable {
    case invalidResponse
    case service(String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse: return "The local control service returned an invalid response."
        case let .service(message): return message
        }
    }
}

final class ConnectionsAPI {
    private let session: URLSession
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    init() {
        let configuration = URLSessionConfiguration.ephemeral
        // Connecting can take up to 14 s on the service (login, then the system
        // document and the preset list); a shorter wait reports a failed
        // connection that then succeeds. Checked by an orban-web test.
        configuration.timeoutIntervalForRequest = 25
        configuration.timeoutIntervalForResource = 30
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        session = URLSession(configuration: configuration)
    }

    deinit { session.invalidateAndCancel() }

    func load(completion: @escaping (Result<ConnectionsContext, Error>) -> Void) {
        get(path: "devices", as: ConnectionsResponse.self) { [weak self] devicesResult in
            guard let self else { return }
            switch devicesResult {
            case let .failure(error): self.finish(.failure(error), completion: completion)
            case let .success(response):
                self.get(path: "sessions", as: SessionList.self) { stateResult in
                    switch stateResult {
                    case let .failure(error): self.finish(.failure(error), completion: completion)
                    case let .success(list):
                        self.finish(.success(ConnectionsContext(
                            devices: response.devices,
                            connectedIDs: Set(list.sessions.filter(\.connected).compactMap(\.key)),
                            groups: response.groups ?? []
                        )), completion: completion)
                    }
                }
            }
        }
    }

    func save(_ edit: ConnectionEdit, completion: @escaping (Result<SavedConnection, Error>) -> Void) {
        post(path: "devices", body: edit, response: SavedConnectionEnvelope.self) { [weak self] result in
            guard let self else { return }
            switch result {
            case let .success(envelope): self.finish(.success(envelope.device), completion: completion)
            case let .failure(error): self.finish(.failure(error), completion: completion)
            }
        }
    }

    func saveCode(id: String, code: String, completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "devices/code", body: ConnectionCodeRequest(id: id, code: code), completion: completion)
    }

    /// Connects the saved connection in its own session; other processors
    /// stay connected.
    func connect(id: String, completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "devices/connect", device: id, body: ConnectionCodeRequest(id: id, code: nil), completion: completion)
    }

    /// Disconnects this processor only.
    func disconnect(id: String, completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "disconnect", device: id, body: EmptyRequest(), completion: completion)
    }

    func group(_ edit: ConnectionGroupEdit, completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "devices/groups", body: edit, completion: completion)
    }

    func delete(id: String, completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "devices/delete", body: ConnectionIDRequest(id: id), completion: completion)
    }

    private func get<T: Decodable & Sendable>(path: String, as type: T.Type, completion: @escaping (Result<T, Error>) -> Void) {
        let request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!, cachePolicy: .reloadIgnoringLocalCacheData)
        session.dataTask(with: request) { [decoder] data, response, error in
            if let error { completion(.failure(error)); return }
            guard let response = response as? HTTPURLResponse, response.statusCode == 200, let data else {
                completion(.failure(Self.responseError(data: data))); return
            }
            do { completion(.success(try decoder.decode(type, from: data))) }
            catch { completion(.failure(error)) }
        }.resume()
    }

    private func postWithoutResponse<T: Encodable>(path: String, device: String? = nil, body: T,
                                                   completion: @escaping (Result<Void, Error>) -> Void) {
        post(path: path, device: device, body: body, response: EmptyResponse.self) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success: self.finish(.success(()), completion: completion)
            case let .failure(error): self.finish(.failure(error), completion: completion)
            }
        }
    }

    private func post<T: Encodable, R: Decodable & Sendable>(path: String, device: String? = nil, body: T, response: R.Type,
                                                            completion: @escaping (Result<R, Error>) -> Void) {
        do {
            var request = URLRequest(url: BackendHealth.api(path, device: device))
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
            request.httpBody = try encoder.encode(body)
            session.dataTask(with: request) { [decoder] data, urlResponse, error in
                if let error { completion(.failure(error)); return }
                guard let response = urlResponse as? HTTPURLResponse,
                      (200..<300).contains(response.statusCode), let data else {
                    completion(.failure(Self.responseError(data: data))); return
                }
                do { completion(.success(try decoder.decode(R.self, from: data))) }
                catch { completion(.failure(error)) }
            }.resume()
        } catch {
            finish(.failure(error), completion: completion)
        }
    }

    private static func responseError(data: Data?) -> Error {
        guard let data,
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let message = object["error"] as? String
        else { return ConnectionsAPIError.invalidResponse }
        return ConnectionsAPIError.service(message)
    }

    private func finish<T>(_ result: Result<T, Error>, completion: @escaping (Result<T, Error>) -> Void) {
        DispatchQueue.main.async { completion(result) }
    }
}

private struct SessionList: Codable, Sendable {
    struct Entry: Codable, Sendable {
        let key: String?
        let connected: Bool
    }

    let sessions: [Entry]
}

private struct SavedConnectionEnvelope: Codable, Sendable {
    let device: SavedConnection
}

private struct ConnectionIDRequest: Codable { let id: String }
private struct ConnectionCodeRequest: Codable { let id: String; let code: String? }
private struct EmptyRequest: Codable {}
private struct EmptyResponse: Codable, Sendable {}
