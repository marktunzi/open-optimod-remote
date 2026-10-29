import Foundation

struct PresetBrowserContext: Sendable {
    let snapshot: DeviceSnapshotPayload

    var currentName: String { snapshot.processing?.name ?? "" }
    var fileFormat: PresetFileFormat { PresetFileFormat.forAdapter(snapshot.adapterID) }
    /// Recall, at every access level that has Presets.
    var ready: Bool { snapshot.allows(\.recall) }
    /// Applying a preset file changes the processing (Modify).
    var canModify: Bool { snapshot.allows(\.modify) }
    /// Restore from backup (the client's Restore from PC).
    var canRestore: Bool { snapshot.allows(\.memory) }
    var canStore: Bool { snapshot.allows(\.presets) && snapshot.capabilities?.presetStore == true }
    var canDelete: Bool {
        snapshot.connected && (snapshot.access?.delete ?? snapshot.allows(\.presets)) && snapshot.capabilities?.presetDelete == true
    }
    /// Saving, renaming and deleting presets on the processor itself.
    var managesOnDevice: Bool {
        snapshot.capabilities?.presetStore == true || snapshot.capabilities?.presetDelete == true
    }

    func isUser(_ name: String) -> Bool {
        snapshot.presets.contains { $0.name == name && $0.kind == "User" }
    }

    /// The user preset on air, with or without changes.
    var onAirUserPreset: String? {
        let base = currentName.hasPrefix("modif ") ? String(currentName.dropFirst(6)) : currentName
        return isUser(base) ? base : nil
    }

    /// Save As stores the on-air processing under a new name.
    var canSaveAs: Bool { canStore && !currentName.isEmpty }
    /// Save replaces the on-air user preset under its own name, as the 5700i client's Save does.
    var replaceName: String? {
        canStore && PresetFileFormat.replacesOnAir(snapshot.adapterID) ? onAirUserPreset : nil
    }
    /// Rename needs the selected user preset on air without changes.
    func canRename(_ selected: String?) -> Bool {
        guard let selected else { return false }
        return canStore && canDelete && isUser(selected) && currentName == selected
    }
    /// Delete needs a user preset that is not on air.
    func canDelete(_ selected: String?) -> Bool {
        guard let selected else { return false }
        return canDelete && isUser(selected) && currentName != selected && currentName != "modif \(selected)"
    }

    /// Why a new preset name cannot be used, or nil when it can.
    func nameProblem(_ name: String) -> String? {
        let max = fileFormat.maxNameLength
        if name.isEmpty { return "Enter a name." }
        if name.count > max { return "Use at most \(max) characters." }
        if name.trimmingCharacters(in: .whitespaces) != name { return "Remove leading and trailing spaces." }
        if name.contains("[") || name.contains("]") || name.unicodeScalars.contains(where: { $0.value < 0x20 || $0.value > 0x7e }) {
            return "Use plain letters, digits and punctuation without brackets."
        }
        if name.contains(where: { "\\/:*?\"<>|".contains($0) }) { return "A name cannot contain \\ / : * ? \" < > or |." }
        if name.lowercased().hasPrefix("modif ") { return "A name cannot start with \"modif \"." }
        if let existing = snapshot.presets.first(where: { $0.name == name }) {
            return existing.kind == "Factory" ? "A factory preset has this name." : "A preset with this name already exists."
        }
        return nil
    }
}

/// The user-preset file name and extension of the connected model, as used by
/// Orban's own PC Remote for that model.
struct PresetFileFormat: Equatable, Sendable {
    let model: String
    let fileExtension: String

    static func forAdapter(_ adapterID: String?) -> PresetFileFormat {
        let id = adapterID ?? ""
        if id.hasPrefix("pc-remote-5500-") { return PresetFileFormat(model: "OPTIMOD 5500", fileExtension: "orb55user") }
        if id.hasPrefix("pc-remote-5500i-") { return PresetFileFormat(model: "OPTIMOD 5500i", fileExtension: "orb55user") }
        if id.hasPrefix("pc-remote-8700hd-") { return PresetFileFormat(model: "OPTIMOD-FM 8700HD", fileExtension: "orb86user") }
        if id.hasPrefix("pc-remote-5700i-") { return PresetFileFormat(model: "OPTIMOD 5700i", fileExtension: "orb57user") }
        return PresetFileFormat(model: "OPTIMOD", fileExtension: "orb")
    }

    /// Longest name the model's own PC Remote accepts for a new preset.
    var maxNameLength: Int { fileExtension == "orb55user" ? 18 : 20 }

    /// Models whose own PC Remote saves the on-air user preset under its own name (5700i, 5500i, 5500, 8700HD Save).
    static func replacesOnAir(_ adapterID: String?) -> Bool { backsUpFiles(adapterID) }

    /// Models whose original PC Remote backs up the unit's own files (setup and user presets).
    static func backsUpFiles(_ adapterID: String?) -> Bool {
        ["pc-remote-5700i-", "pc-remote-5500i-", "pc-remote-5500-", "pc-remote-8700hd-"].contains { adapterID?.hasPrefix($0) == true }
    }

    /// Files the 5500i, 5500 and 8700HD clients' Restore sends back as files, next to the user presets.
    static func restoresAsFiles(_ adapterID: String?) -> [String] {
        if adapterID?.hasPrefix("pc-remote-5500i-") == true { return [".orb55setup", ".orbsetup", ".orb55autom", ".orb55net"] }
        // The 5500 has no network file; its setup file is Setup.orb55setup.
        if adapterID?.hasPrefix("pc-remote-5500-") == true { return [".orb55setup", ".orb55autom"] }
        // The 8700HD client's Restore is the 5500's, with .orb86 names.
        if adapterID?.hasPrefix("pc-remote-8700hd-") == true { return [".orb86setup", ".orb86autom"] }
        return []
    }
}

/// A file of the original PC Remote's backup folder.
struct PresetBackupFile: Codable, Equatable, Sendable {
    let name: String
    let document: String
    /// Restore only: the file's modification time, milliseconds since 1970.
    var modified: UInt64?
}

enum PresetBackup {
    /// Sorts files chosen for a restore into the setup file and user preset
    /// files; anything else is refused.
    static func sort(_ files: [PresetBackupFile], adapterID: String?) throws -> (setup: PresetBackupFile?, presets: [PresetBackupFile]) {
        let fileExtension = "." + PresetFileFormat.forAdapter(adapterID).fileExtension
        var setup: PresetBackupFile?
        var presets: [PresetBackupFile] = []
        for file in files {
            let lower = file.name.lowercased()
            if PresetFileFormat.restoresAsFiles(adapterID).contains(where: { lower.hasSuffix($0) }) {
                presets.append(file)
            } else if lower == "setup.orb57setup" && PresetFileFormat.backsUpFiles(adapterID) {
                if setup != nil { throw SystemSettingsAPIError.service("Choose one setup file.") }
                setup = file
            } else if lower.hasSuffix(fileExtension) {
                presets.append(file)
            } else {
                throw SystemSettingsAPIError.service("\(file.name) is not a backup file of this processor.")
            }
        }
        return (setup, presets)
    }

    /// Checks that a JSON backup belongs to the connected model and firmware
    /// and returns its system document.
    static func systemDocument(of data: Data, adapterID: String?) throws -> String {
        let notBackup = SystemSettingsAPIError.service("This is not an Open Optimod Remote backup file.")
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              json["format"] as? String == "open-optimod-backup", json["version"] as? Int == 1,
              let system = (json["system"] as? [String: Any])?["document"] as? String,
              (json["processing"] as? [String: Any])?["document"] is String
        else { throw notBackup }
        guard let adapterID, json["adapter_id"] as? String == adapterID else {
            let model = json["model"] as? String ?? "another processor"
            let firmware = json["adapter_id"] as? String ?? "unknown firmware"
            throw SystemSettingsAPIError.service("This backup was made on \(model) (\(firmware)). Restore it on the same model and firmware.")
        }
        return system
    }

    /// An uncompressed ZIP of `files` in a folder named `folder`, as Finder opens it.
    /// Entries carry `date` (local time), so a restore from the unzipped
    /// folder sends the unit the backup's time, not 1980.
    static func zip(folder: String, files: [(name: String, data: Data)], date: Date = Date()) -> Data {
        let c = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        let dosTime = (c.hour ?? 0) << 11 | (c.minute ?? 0) << 5 | (c.second ?? 0) / 2
        let dosDate = max(0, (c.year ?? 1980) - 1980) << 9 | (c.month ?? 1) << 5 | (c.day ?? 1)
        var local = Data()
        var central = Data()
        func u16(_ value: Int, _ data: inout Data) { var v = UInt16(truncatingIfNeeded: value).littleEndian; data.append(Data(bytes: &v, count: 2)) }
        func u32(_ value: UInt32, _ data: inout Data) { var v = value.littleEndian; data.append(Data(bytes: &v, count: 4)) }
        for file in files {
            let name = Data("\(folder)/\(file.name)".utf8)
            let crc = crc32(file.data)
            let offset = UInt32(local.count)
            let size = UInt32(file.data.count)
            u32(0x0403_4b50, &local); u16(20, &local); u16(0, &local); u16(0, &local); u16(dosTime, &local); u16(dosDate, &local)
            u32(crc, &local); u32(size, &local); u32(size, &local); u16(name.count, &local); u16(0, &local)
            local.append(name); local.append(file.data)
            u32(0x0201_4b50, &central); u16(20, &central); u16(20, &central); u16(0, &central); u16(0, &central)
            u16(dosTime, &central); u16(dosDate, &central); u32(crc, &central); u32(size, &central); u32(size, &central)
            u16(name.count, &central); u16(0, &central); u16(0, &central); u16(0, &central); u16(0, &central)
            u32(0, &central); u32(offset, &central)
            central.append(name)
        }
        var end = Data()
        u32(0x0605_4b50, &end); u16(0, &end); u16(0, &end); u16(files.count, &end); u16(files.count, &end)
        u32(UInt32(central.count), &end); u32(UInt32(local.count), &end); u16(0, &end)
        return local + central + end
    }

    private static let crcTable: [UInt32] = (0..<256).map { n in
        var c = UInt32(n)
        for _ in 0..<8 { c = c & 1 != 0 ? 0xedb8_8320 ^ (c >> 1) : c >> 1 }
        return c
    }

    static func crc32(_ data: Data) -> UInt32 {
        var c: UInt32 = 0xffff_ffff
        for byte in data { c = crcTable[Int((c ^ UInt32(byte)) & 0xff)] ^ (c >> 8) }
        return c ^ 0xffff_ffff
    }

    /// The name the web interface also gives a backup: letters, digits, space, `._-`.
    static func safeFilename(_ name: String) -> String {
        let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 ._-")
        let clean = String(name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "_" })
            .trimmingCharacters(in: .whitespaces)
        return clean.isEmpty ? "preset" : clean
    }
}

/// The recallable presets: the modified on-air preset is shown as on air,
/// never as a preset of its own.
enum PresetBrowserFilter: String, CaseIterable {
    case all = "All Presets"
    case factory = "Factory"
    case user = "User"

    func includes(_ preset: DevicePresetPayload) -> Bool {
        switch self {
        case .all: return preset.kind.caseInsensitiveCompare("Unsaved") != .orderedSame
        case .factory: return preset.kind.caseInsensitiveCompare("Factory") == .orderedSame
        case .user: return preset.kind.caseInsensitiveCompare("User") == .orderedSame
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
    /// The processor whose session this reads and writes.
    let deviceID: String?

    init(deviceID: String?) {
        self.deviceID = deviceID
        let configuration = URLSessionConfiguration.ephemeral
        // Recall can take up to 16 s, and applying a preset file writes every
        // differing field before reading the document back (about 0.15 s per
        // field). Checked by an orban-web test.
        configuration.timeoutIntervalForRequest = 120
        configuration.timeoutIntervalForResource = 180
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

    /// Any other preset or backup call, as JSON in and out.
    func json(_ path: String, body: [String: Any]? = nil, completion: @escaping (Result<[String: Any], Error>) -> Void) {
        var request = URLRequest(url: BackendHealth.api(path, device: deviceID))
        request.cachePolicy = .reloadIgnoringLocalCacheData
        if let body {
            request.httpMethod = "POST"
            request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        }
        session.dataTask(with: request) { data, response, error in
            let result: Result<[String: Any], Error>
            let object = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? [:]
            if let error { result = .failure(error) }
            else if let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) { result = .success(object) }
            else { result = .failure(SystemSettingsAPIError.service(object["error"] as? String ?? "The preset operation failed.")) }
            DispatchQueue.main.async { completion(result) }
        }.resume()
    }

    private func request<Body: Encodable, Output: Decodable>(
        path: String,
        method: String,
        body: Body?,
        as _: Output.Type,
        completion: @escaping (Result<Output, Error>) -> Void
    ) {
        var request = URLRequest(url: BackendHealth.api(path, device: deviceID))
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
