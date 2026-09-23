import Foundation

struct PresetBrowserContext: Sendable {
    let snapshot: DeviceSnapshotPayload

    var currentName: String { snapshot.processing?.name ?? "" }
}

enum PresetBrowserFilter: String, CaseIterable {
    case all = "All Presets"
    case factory = "Factory"
    case user = "User"
    case modified = "Modified"

    func includes(_ preset: DevicePresetPayload) -> Bool {
        switch self {
        case .all: return true
        case .factory: return preset.kind.caseInsensitiveCompare("Factory") == .orderedSame
        case .user: return preset.kind.caseInsensitiveCompare("User") == .orderedSame
        case .modified: return preset.kind.caseInsensitiveCompare("Unsaved") == .orderedSame
        }
    }
}

struct PresetRecallRequest: Encodable, Equatable {
    let name: String
    let expectedName: String
    let confirmed: Bool
    let expectedHost: String
    let expectedSession: String

    enum CodingKeys: String, CodingKey {
        case name, confirmed
        case expectedName = "expected_name"
        case expectedHost = "expected_host"
        case expectedSession = "expected_session"
    }
}

struct PresetFilePayload: Decodable, Equatable, Sendable {
    let name: String
    let document: String
}

struct PresetApplyRequest: Encodable, Equatable {
    let document: String
    let expectedName: String
    let confirmed: Bool
    let expectedHost: String
    let expectedSession: String

    enum CodingKeys: String, CodingKey {
        case document, confirmed
        case expectedName = "expected_name"
        case expectedHost = "expected_host"
        case expectedSession = "expected_session"
    }
}

final class PresetsAPI {
    private let session: URLSession
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 12
        configuration.timeoutIntervalForResource = 18
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        session = URLSession(configuration: configuration)
    }

    deinit { session.invalidateAndCancel() }

    func load(completion: @escaping (Result<PresetBrowserContext, Error>) -> Void) {
        request(path: "state", method: "GET", body: Optional<Int>.none, as: DeviceSnapshotPayload.self) { result in
            DispatchQueue.main.async { completion(result.map(PresetBrowserContext.init(snapshot:))) }
        }
    }

    func recall(_ requestBody: PresetRecallRequest, completion: @escaping (Result<Void, Error>) -> Void) {
        request(path: "presets/recall", method: "POST", body: requestBody, as: APIConfirmation.self) { result in
            DispatchQueue.main.async { completion(result.map { _ in () }) }
        }
    }

    func currentFile(completion: @escaping (Result<PresetFilePayload, Error>) -> Void) {
        request(path: "presets/current-file", method: "GET", body: Optional<Int>.none, as: PresetFilePayload.self) { result in
            DispatchQueue.main.async { completion(result) }
        }
    }

    func apply(_ requestBody: PresetApplyRequest, completion: @escaping (Result<Void, Error>) -> Void) {
        request(path: "presets/apply-file", method: "POST", body: requestBody, as: APIConfirmation.self) { result in
            DispatchQueue.main.async { completion(result.map { _ in () }) }
        }
    }

    private func request<Body: Encodable, Output: Decodable>(
        path: String,
        method: String,
        body: Body?,
        as _: Output.Type,
        completion: @escaping (Result<Output, Error>) -> Void
    ) {
        var request = URLRequest(url: URL(string: "\(BackendHealth.origin)/api/\(path)")!)
        request.httpMethod = method
        request.cachePolicy = .reloadIgnoringLocalCacheData
        if method != "GET" { request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin") }
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            do { request.httpBody = try encoder.encode(body) }
            catch { completion(.failure(error)); return }
        }
        session.dataTask(with: request) { [decoder] data, response, error in
            if let error { completion(.failure(error)); return }
            guard let http = response as? HTTPURLResponse, let data else {
                completion(.failure(SystemSettingsAPIError.invalidResponse)); return
            }
            guard (200..<300).contains(http.statusCode) else {
                let message = (try? decoder.decode(APIErrorPayload.self, from: data).error)
                    ?? "The preset operation failed."
                completion(.failure(SystemSettingsAPIError.service(message))); return
            }
            do { completion(.success(try decoder.decode(Output.self, from: data))) }
            catch { completion(.failure(error)) }
        }.resume()
    }
}

private struct APIConfirmation: Decodable { let ok: Bool }
private struct APIErrorPayload: Decodable { let error: String }
