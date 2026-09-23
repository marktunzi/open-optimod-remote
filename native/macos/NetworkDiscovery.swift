import Foundation
import Network
import Darwin

struct DiscoveredOptimod: Equatable, Sendable {
    let host: String
    let name: String
    let banner: String
    let model: String
}

enum OptimodDiscoveryLogic {
    static func candidateHosts(address: String, netmask: String, maximum: Int = 254) -> [String] {
        guard let addressValue = ipv4(address), let maskValue = ipv4(netmask) else { return [] }
        let hostBits = ~maskValue
        let count = hostBits > 1 ? UInt64(hostBits) - 1 : 0
        let network: UInt32
        let broadcast: UInt32
        if count > UInt64(maximum) {
            network = addressValue & 0xffff_ff00
            broadcast = network | 0xff
        } else {
            network = addressValue & maskValue
            broadcast = network | hostBits
        }
        guard broadcast > network + 1 else { return [] }
        return ((network + 1)..<broadcast)
            .filter { $0 != addressValue }
            .prefix(maximum)
            .map(ipv4String)
    }

    static func is5700iBanner(_ banner: String) -> Bool {
        supportedProcessor(banner)?.model == "optimod-5700i"
    }

    static func supportedProcessor(_ banner: String) -> (name: String, model: String)? {
        let normalized = banner.lowercased()
        for (needle, name, model) in [
            ("orban optimod 5700i", "OPTIMOD 5700i", "optimod-5700i"),
            ("orban optimod 5500i", "OPTIMOD 5500i", "optimod-5500i"),
            ("orban optimod 5500", "OPTIMOD 5500", "optimod-5500"),
            ("orban optimod 5700fm", "OPTIMOD 5700 FM", "optimod-5700-fm"),
            ("orban optimod 5700hd", "OPTIMOD 5700 HD", "optimod-5700-hd"),
            ("orban optimod 8500", "OPTIMOD 8500", "optimod-8500"),
            ("orban optimod 6300", "OPTIMOD 6300", "optimod-6300"),
            ("orban optimod 8600", "OPTIMOD 8600", "optimod-8600"),
            ("orban optimod 8700i", "OPTIMOD 8700i", "optimod-8700i"),
            ("orban optimod 9300", "OPTIMOD 9300", "optimod-9300"),
            ("orban optimod 9400", "OPTIMOD 9400", "optimod-9400"),
        ] where normalized.contains(needle) {
            return (name, model)
        }
        return nil
    }

    private static func ipv4(_ text: String) -> UInt32? {
        let parts = text.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 4 else { return nil }
        var value: UInt32 = 0
        for part in parts {
            guard let byte = UInt8(part) else { return nil }
            value = (value << 8) | UInt32(byte)
        }
        return value
    }

    private static func ipv4String(_ value: UInt32) -> String {
        [24, 16, 8, 0].map { String((value >> UInt32($0)) & 0xff) }.joined(separator: ".")
    }
}

final class OptimodNetworkDiscovery {
    private let workQueue = DispatchQueue(label: "OpenOptimodRemote.NetworkDiscovery", qos: .userInitiated)
    private let probeQueue = DispatchQueue(label: "OpenOptimodRemote.NetworkProbe", qos: .utility, attributes: .concurrent)
    private var cancelled = false
    private let cancellationLock = NSLock()

    func cancel() {
        cancellationLock.lock()
        cancelled = true
        cancellationLock.unlock()
    }

    func discover(found: @escaping (DiscoveredOptimod) -> Void, completion: @escaping () -> Void) {
        cancellationLock.lock()
        cancelled = false
        cancellationLock.unlock()
        workQueue.async { [weak self] in
            guard let self else { return }
            let candidates = Array(Set(Self.localNetworks().flatMap {
                OptimodDiscoveryLogic.candidateHosts(address: $0.address, netmask: $0.mask)
            })).sorted { $0.localizedStandardCompare($1) == .orderedAscending }

            for offset in stride(from: 0, to: candidates.count, by: 32) {
                if self.isCancelled { break }
                let end = min(offset + 32, candidates.count)
                let group = DispatchGroup()
                for host in candidates[offset..<end] {
                    group.enter()
                    self.probe(host: host) { result in
                        if let result { DispatchQueue.main.async { found(result) } }
                        group.leave()
                    }
                }
                group.wait()
            }
            DispatchQueue.main.async(execute: completion)
        }
    }

    private var isCancelled: Bool {
        cancellationLock.lock()
        defer { cancellationLock.unlock() }
        return cancelled
    }

    private func probe(host: String, completion: @escaping (DiscoveredOptimod?) -> Void) {
        guard let port = NWEndpoint.Port(rawValue: 23) else { completion(nil); return }
        let connection = NWConnection(host: NWEndpoint.Host(host), port: port, using: .tcp)
        let lock = NSLock()
        var completed = false
        func finish(_ result: DiscoveredOptimod?) {
            lock.lock()
            guard !completed else { lock.unlock(); return }
            completed = true
            lock.unlock()
            connection.cancel()
            completion(result)
        }
        connection.stateUpdateHandler = { state in
            switch state {
            case .ready:
                connection.receive(minimumIncompleteLength: 1, maximumLength: 512) { data, _, _, _ in
                    guard let data, let banner = String(data: data, encoding: .utf8),
                          let processor = OptimodDiscoveryLogic.supportedProcessor(banner) else { finish(nil); return }
                    finish(DiscoveredOptimod(
                        host: host,
                        name: processor.name,
                        banner: banner.trimmingCharacters(in: .whitespacesAndNewlines),
                        model: processor.model
                    ))
                }
            case .failed, .cancelled: finish(nil)
            default: break
            }
        }
        connection.start(queue: probeQueue)
        probeQueue.asyncAfter(deadline: .now() + 0.75) { finish(nil) }
    }

    private static func localNetworks() -> [(address: String, mask: String)] {
        var pointer: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&pointer) == 0, let first = pointer else { return [] }
        defer { freeifaddrs(pointer) }
        var result: [(String, String)] = []
        var cursor: UnsafeMutablePointer<ifaddrs>? = first
        while let interface = cursor {
            let flags = Int32(interface.pointee.ifa_flags)
            if let address = interface.pointee.ifa_addr,
               let mask = interface.pointee.ifa_netmask,
               address.pointee.sa_family == UInt8(AF_INET),
               flags & IFF_UP != 0,
               flags & IFF_LOOPBACK == 0 {
                let addressText = text(address)
                let maskText = text(mask)
                if !addressText.isEmpty, !maskText.isEmpty { result.append((addressText, maskText)) }
            }
            cursor = interface.pointee.ifa_next
        }
        return result
    }

    private static func text(_ address: UnsafePointer<sockaddr>) -> String {
        var copy = address.pointee
        var buffer = [CChar](repeating: 0, count: Int(NI_MAXHOST))
        let status = getnameinfo(&copy, socklen_t(copy.sa_len), &buffer, socklen_t(buffer.count), nil, 0, NI_NUMERICHOST)
        return status == 0 ? String(cString: buffer) : ""
    }
}
