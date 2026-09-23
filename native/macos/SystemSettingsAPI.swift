import Foundation

enum DeviceScalar: Codable, Equatable, Sendable {
    case integer(Int)
    case number(Double)
    case string(String)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let value = try? container.decode(Int.self) {
            self = .integer(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else {
            self = .string(try container.decode(String.self))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case let .integer(value): try container.encode(value)
        case let .number(value): try container.encode(value)
        case let .string(value): try container.encode(value)
        }
    }

    var numeric: Double? {
        switch self {
        case let .integer(value): return Double(value)
        case let .number(value): return value
        case let .string(value): return Double(value)
        }
    }

    var text: String {
        switch self {
        case let .integer(value): return String(value)
        case let .number(value): return String(value)
        case let .string(value): return value
        }
    }
}

struct DeviceValuePayload: Codable, Equatable, Sendable {
    let type: String
    let value: DeviceScalar

    static func text(_ value: String) -> DeviceValuePayload {
        DeviceValuePayload(type: "Text", value: .string(value))
    }

    var numeric: Double? {
        guard type == "Int" || type == "Cent" else { return nil }
        guard let number = value.numeric else { return nil }
        return type == "Cent" ? number / 100 : number
    }

    var displayText: String {
        if let numeric {
            return Self.format(numeric)
        }
        return value.text
    }

    static func format(_ value: Double, decimals: Int = 6) -> String {
        let rounded = value.rounded()
        if abs(value - rounded) < 0.000_000_1 { return String(Int(rounded)) }
        var text = String(format: "%.*f", decimals, value)
        while text.last == "0" { text.removeLast() }
        if text.last == "." { text.removeLast() }
        return text
    }
}

struct DeviceFieldPayload: Codable, Equatable, Sendable {
    let index: Int
    let value: DeviceValuePayload
}

struct DeviceDocumentPayload: Codable, Sendable {
    let name: String
    let fields: [String: DeviceFieldPayload]
}

struct DevicePresetPayload: Codable, Sendable {
    let name: String
    let kind: String
}

struct DeviceSnapshotPayload: Codable, Sendable {
    let connected: Bool
    let firmware: String
    let host: String
    let system: DeviceDocumentPayload?
    let processing: DeviceDocumentPayload?
    let error: String?
    let writeEnabled: Bool
    let sessionID: String?
    let presets: [DevicePresetPayload]

    enum CodingKeys: String, CodingKey {
        case connected, firmware, host, system, processing, error, presets
        case writeEnabled = "write_enabled"
        case sessionID = "session_id"
    }
}

struct IntegerRangePayload: Codable, Sendable {
    let min: Int
    let max: Int
}

struct SampleDelayPayload: Codable, Sendable {
    let maxIndex: Int
    let offset: Int
    let rate: Int

    enum CodingKeys: String, CodingKey {
        case offset, rate
        case maxIndex = "max_index"
    }
}

struct DeviceDefinitionPayload: Codable, Sendable {
    let scope: String
    let name: String
    let values: [DeviceValuePayload]
    let unit: String
    let integerRange: IntegerRangePayload?
    let sampleDelay: SampleDelayPayload?

    enum CodingKeys: String, CodingKey {
        case scope, name, values, unit
        case integerRange = "integer_range"
        case sampleDelay = "sample_delay"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        scope = try container.decode(String.self, forKey: .scope)
        name = try container.decode(String.self, forKey: .name)
        values = try container.decodeIfPresent([DeviceValuePayload].self, forKey: .values) ?? []
        unit = try container.decodeIfPresent(String.self, forKey: .unit) ?? ""
        integerRange = try container.decodeIfPresent(IntegerRangePayload.self, forKey: .integerRange)
        sampleDelay = try container.decodeIfPresent(SampleDelayPayload.self, forKey: .sampleDelay)
    }

    func deviceIndex(matching aliases: [String]) -> Int? {
        let wanted = Set(aliases.map(Self.normalize))
        return values.firstIndex { wanted.contains(Self.normalize($0.displayText)) }
    }

    func numericEntries(minimum: Double, maximum: Double) -> [(index: Int, value: Double)] {
        values.enumerated().compactMap { index, payload in
            guard let value = payload.numeric, value >= minimum - 0.000_001, value <= maximum + 0.000_001 else {
                return nil
            }
            return (index, value)
        }
    }

    func deviceIndex(numericValue: Double, tolerance: Double = 0.000_001) -> Int? {
        if let integerRange {
            let rounded = numericValue.rounded()
            guard abs(numericValue - rounded) <= tolerance else { return nil }
            let index = Int(rounded)
            return (integerRange.min...integerRange.max).contains(index) ? index : nil
        }
        return values.firstIndex { value in
            value.numeric.isSomeAnd { abs($0 - numericValue) <= tolerance }
        }
    }

    private static func normalize(_ value: String) -> String {
        value.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: Locale(identifier: "en_US_POSIX"))
            .replacingOccurrences(of: " ", with: "")
            .replacingOccurrences(of: ".", with: "")
            .replacingOccurrences(of: "_", with: "")
            .replacingOccurrences(of: "-", with: "")
            .lowercased()
    }
}

private extension Optional {
    func isSomeAnd(_ predicate: (Wrapped) -> Bool) -> Bool {
        guard let self else { return false }
        return predicate(self)
    }
}

struct DeviceProfilePayload: Codable, Sendable {
    let model: String
    let firmware: String
    let fields: [DeviceDefinitionPayload]
}

struct SystemSettingsContext: Sendable {
    let snapshot: DeviceSnapshotPayload
    let definitions: [String: DeviceDefinitionPayload]

    var editable: Bool {
        snapshot.connected && snapshot.writeEnabled && snapshot.sessionID != nil
    }
}

enum SystemSettingsAPIError: LocalizedError, Sendable {
    case invalidResponse
    case service(String)
    case unavailable(String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse: return "The local control service returned an invalid response."
        case let .service(message), let .unavailable(message): return message
        }
    }
}

final class SystemSettingsAPI {
    private let session: URLSession
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()
    private var cachedProfile: DeviceProfilePayload?

    init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 5
        configuration.timeoutIntervalForResource = 8
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        session = URLSession(configuration: configuration)
    }

    deinit {
        session.invalidateAndCancel()
    }

    func load(completion: @escaping (Result<SystemSettingsContext, Error>) -> Void) {
        get(path: "state", as: DeviceSnapshotPayload.self) { [weak self] stateResult in
            guard let self else { return }
            switch stateResult {
            case let .failure(error): self.finish(.failure(error), completion: completion)
            case let .success(snapshot):
                if let profile = self.cachedProfile {
                    let systemDefinitions = Dictionary(uniqueKeysWithValues: profile.fields
                        .filter { $0.scope == "System" }
                        .map { ($0.name, $0) })
                    self.finish(.success(SystemSettingsContext(snapshot: snapshot, definitions: systemDefinitions)), completion: completion)
                    return
                }
                self.get(path: "profile", as: DeviceProfilePayload.self) { profileResult in
                    switch profileResult {
                    case let .failure(error): self.finish(.failure(error), completion: completion)
                    case let .success(profile):
                        self.cachedProfile = profile
                        let systemDefinitions = Dictionary(uniqueKeysWithValues: profile.fields
                            .filter { $0.scope == "System" }
                            .map { ($0.name, $0) })
                        self.finish(.success(SystemSettingsContext(snapshot: snapshot, definitions: systemDefinitions)), completion: completion)
                    }
                }
            }
        }
    }

    func change(
        context: SystemSettingsContext,
        fieldName: String,
        index: Int,
        requestedText: String?,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        guard context.snapshot.connected,
              context.snapshot.writeEnabled,
              let sessionID = context.snapshot.sessionID,
              let expected = context.snapshot.system?.fields[fieldName]
        else {
            finish(.failure(SystemSettingsAPIError.unavailable("Connect with write access before changing settings.")), completion: completion)
            return
        }
        let body = ChangeRequest(
            scope: "System",
            name: fieldName,
            index: index,
            expected: expected,
            requested: requestedText.map(DeviceValuePayload.text),
            expectedHost: context.snapshot.host,
            expectedSession: sessionID
        )
        post(path: "change", body: body) { [weak self] result in
            guard let self else { return }
            switch result {
            case let .failure(error): self.finish(.failure(error), completion: completion)
            case .success: self.finish(.success(()), completion: completion)
            }
        }
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

    private func post<T: Encodable>(path: String, body: T, completion: @escaping (Result<Void, Error>) -> Void) {
        do {
            var request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
            request.httpBody = try encoder.encode(body)
            session.dataTask(with: request) { data, response, error in
                if let error { completion(.failure(error)); return }
                guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
                    completion(.failure(Self.responseError(data: data))); return
                }
                completion(.success(()))
            }.resume()
        } catch {
            completion(.failure(error))
        }
    }

    private static func responseError(data: Data?) -> Error {
        guard let data,
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let message = object["error"] as? String
        else { return SystemSettingsAPIError.invalidResponse }
        return SystemSettingsAPIError.service(message)
    }

    private func finish<T>(_ result: Result<T, Error>, completion: @escaping (Result<T, Error>) -> Void) {
        DispatchQueue.main.async { completion(result) }
    }
}

private struct ChangeRequest: Encodable {
    let scope: String
    let name: String
    let index: Int
    let expected: DeviceFieldPayload
    let requested: DeviceValuePayload?
    let expectedHost: String
    let expectedSession: String

    enum CodingKeys: String, CodingKey {
        case scope, name, index, expected, requested
        case expectedHost = "expected_host"
        case expectedSession = "expected_session"
    }
}
