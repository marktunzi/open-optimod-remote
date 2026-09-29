import Foundation

@main
enum NetworkDiscoveryTests {
    static func main() {
        let hosts = OptimodDiscoveryLogic.candidateHosts(address: "192.168.50.20", netmask: "255.255.255.0")
        precondition(hosts.count == 253)
        precondition(hosts.first == "192.168.50.1")
        precondition(hosts.last == "192.168.50.254")
        precondition(!hosts.contains("192.168.50.20"))
        precondition(OptimodDiscoveryLogic.is5700iBanner("Orban Optimod 5700i V 3.0.1.20\r\n"))
        for (banner, expected) in [
            ("Orban Optimod 5500i V 3.1", "OPTIMOD 5500i"),
            ("Orban Optimod 5500 V 2.0", "OPTIMOD 5500"),
            ("Orban Optimod 5700FM V 1.0", "OPTIMOD 5700 FM"),
            ("Orban Optimod 5700HD V 1.0", "OPTIMOD 5700 HD"),
            ("Orban Optimod 8500 V 4.0", "OPTIMOD 8500"),
            ("Orban Optimod 6300 V 4.1", "OPTIMOD 6300"),
            ("Orban Optimod 8600 V 4.5", "OPTIMOD 8600"),
            ("Orban Optimod 8700i V 1.5", "OPTIMOD 8700i"),
            ("Welcome to the Orban Optimod-FM 8700HD.", "OPTIMOD-FM 8700HD"),
            ("Orban Optimod 9300 V 2.1", "OPTIMOD 9300"),
            ("Orban Optimod 9400 V 2.0", "OPTIMOD 9400"),
        ] {
            precondition(OptimodDiscoveryLogic.supportedProcessor(banner)?.name == expected)
        }
        precondition(!OptimodDiscoveryLogic.is5700iBanner("SSH-2.0-OpenSSH"))
        precondition(OptimodDiscoveryLogic.supportedProcessor("SSH-2.0-OpenSSH") == nil)
        print("NetworkDiscoveryTests passed")
    }
}
