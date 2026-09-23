import Foundation

enum ProcessorModel: String, Codable, CaseIterable, Sendable {
    case auto
    case optimod5700i = "optimod-5700i"
    case optimod5500i = "optimod-5500i"
    case optimod5500 = "optimod-5500"
    case optimod5700FM = "optimod-5700-fm"
    case optimod5700HD = "optimod-5700-hd"
    case optimod8500 = "optimod-8500"
    case optimod6300 = "optimod-6300"
    case optimod8600 = "optimod-8600"
    case optimod8700i = "optimod-8700i"
    case optimod9300 = "optimod-9300"
    case optimod9400 = "optimod-9400"

    var label: String {
        switch self {
        case .auto: "Auto Detect"
        case .optimod5700i: "OPTIMOD 5700i"
        case .optimod5500i: "OPTIMOD 5500i"
        case .optimod5500: "OPTIMOD 5500"
        case .optimod5700FM: "OPTIMOD 5700 FM"
        case .optimod5700HD: "OPTIMOD 5700 HD"
        case .optimod8500: "OPTIMOD 8500"
        case .optimod6300: "OPTIMOD 6300"
        case .optimod8600: "OPTIMOD 8600"
        case .optimod8700i: "OPTIMOD 8700i"
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

    var processorModel: ProcessorModel { model ?? .auto }

    init(id: String, name: String, host: String, port: Int, terminalPort: Int, hasCode: Bool, model: ProcessorModel = .auto) {
        self.id = id
        self.name = name
        self.host = host
        self.port = port
        self.terminalPort = terminalPort
        self.hasCode = hasCode
        self.model = model
    }

    enum CodingKeys: String, CodingKey {
        case id, name, host, port, model
        case terminalPort = "terminal_port"
        case hasCode = "has_code"
    }
}

struct ConnectionsResponse: Codable, Sendable {
    let devices: [SavedConnection]
}

struct ConnectionEdit: Codable, Sendable {
    let id: String?
    let name: String
    let host: String
    let port: Int
    let terminalPort: Int
    let model: ProcessorModel

    init(id: String?, name: String, host: String, port: Int, terminalPort: Int, model: ProcessorModel = .auto) {
        self.id = id
        self.name = name
        self.host = host
        self.port = port
        self.terminalPort = terminalPort
        self.model = model
    }

    enum CodingKeys: String, CodingKey {
        case id, name, host, port, model
        case terminalPort = "terminal_port"
    }
}

struct ConnectionsContext: Sendable {
    let devices: [SavedConnection]
    let connected: Bool
    let activeID: String?
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

    static func mustDisconnect(connected: Bool, activeID: String?, deletingID: String) -> Bool {
        connected && activeID == deletingID
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
        configuration.timeoutIntervalForRequest = 8
        configuration.timeoutIntervalForResource = 12
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
                self.get(path: "state", as: ConnectionState.self) { stateResult in
                    switch stateResult {
                    case let .failure(error): self.finish(.failure(error), completion: completion)
                    case let .success(state):
                        self.finish(.success(ConnectionsContext(
                            devices: response.devices,
                            connected: state.connected,
                            activeID: state.deviceID
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

    func connect(id: String, completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "devices/connect", body: ConnectionCodeRequest(id: id, code: nil), completion: completion)
    }

    func disconnect(completion: @escaping (Result<Void, Error>) -> Void) {
        postWithoutResponse(path: "disconnect", body: EmptyRequest(), completion: completion)
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

    private func postWithoutResponse<T: Encodable>(path: String, body: T, completion: @escaping (Result<Void, Error>) -> Void) {
        post(path: path, body: body, response: EmptyResponse.self) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success: self.finish(.success(()), completion: completion)
            case let .failure(error): self.finish(.failure(error), completion: completion)
            }
        }
    }

    private func post<T: Encodable, R: Decodable & Sendable>(path: String, body: T, response: R.Type, completion: @escaping (Result<R, Error>) -> Void) {
        do {
            var request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!)
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

private struct ConnectionState: Codable, Sendable {
    let connected: Bool
    let deviceID: String?

    enum CodingKeys: String, CodingKey {
        case connected
        case deviceID = "device_id"
    }
}

private struct SavedConnectionEnvelope: Codable, Sendable {
    let device: SavedConnection
}

private struct ConnectionIDRequest: Codable { let id: String }
private struct ConnectionCodeRequest: Codable { let id: String; let code: String? }
private struct EmptyRequest: Codable {}
private struct EmptyResponse: Codable, Sendable {}
