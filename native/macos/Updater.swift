import AppKit
import Foundation
import Sparkle

/// Signed automatic updates (Sparkle 2). The feed and every update are signed
/// with the EdDSA key whose public half is SUPublicEDKey in Info.plist; Sparkle
/// refuses anything else. An update never ends a processor session by itself:
/// with processors connected it asks first (Disconnect and Install, or Later)
/// and otherwise waits until the app quits.
final class AppUpdater: NSObject, SPUUpdaterDelegate, NSMenuDelegate, NSMenuItemValidation {
    static let shared = AppUpdater()

    /// The last thing that happened, in words (shown in the menu's tooltip and the test state).
    private(set) var lastResult = "Not checked yet."
    /// The version that is downloaded and waits to install.
    private(set) var pendingVersion: String?
    private var pendingInstall: (() -> Void)?
    private var asking = false
    private var controller: SPUStandardUpdaterController!

    private lazy var urlSession: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 2
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: configuration)
    }()

    private override init() {
        super.init()
        controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: self, userDriverDelegate: nil)
    }

    /// Starts the scheduled checks; called once the local service is ready.
    func start() {
        do { try controller.updater.start() } catch { note("Updates could not start: \(error.localizedDescription)") }
    }

    var canCheck: Bool { controller.updater.canCheckForUpdates }

    @objc func checkForUpdates(_ sender: Any?) { controller.checkForUpdates(sender) }

    func checkInBackground() { controller.updater.checkForUpdatesInBackground() }

    /// Install Update…: the waiting update installs now, after the same question.
    @objc func installPendingUpdate(_ sender: Any?) {
        guard let install = pendingInstall, let version = pendingVersion else { return }
        installWhenIdle(version: version, install: install)
    }

    var state: [String: Any] {
        ["lastResult": lastResult, "pendingVersion": pendingVersion ?? NSNull(), "asking": asking,
         "version": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") ?? "",
         "build": Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") ?? ""]
    }

    private func note(_ text: String) {
        lastResult = text
        NSLog("Open Optimod Remote updates: %@", text)
    }

    // MARK: Menu

    /// Install Update… shows only while an update waits.
    func menuNeedsUpdate(_ menu: NSMenu) {
        guard let item = menu.items.first(where: { $0.action == #selector(installPendingUpdate(_:)) }) else { return }
        item.isHidden = pendingVersion == nil
        item.title = pendingVersion.map { "Install Update \($0)…" } ?? "Install Update…"
    }

    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(checkForUpdates(_:)) { return canCheck }
        if menuItem.action == #selector(installPendingUpdate(_:)) { return pendingInstall != nil && !asking }
        return true
    }

    // MARK: Waiting for the sessions

    private func activeProcessors(_ completion: @escaping ([UpdateGate.Processor]?) -> Void) {
        urlSession.dataTask(with: BackendHealth.api("sessions")) { data, response, _ in
            let ok = (response as? HTTPURLResponse)?.statusCode == 200
            // No service = no session to protect (it is not running).
            let active = ok ? UpdateGate.activeProcessors(sessionsJSON: data) : (response == nil ? [] : nil)
            DispatchQueue.main.async { completion(active) }
        }.resume()
    }

    /// Installs at once when no processor is connected; otherwise asks.
    private func installWhenIdle(version: String, install: @escaping () -> Void) {
        guard !asking else { return }
        activeProcessors { [weak self] active in
            guard let self else { return }
            guard let active else {
                self.keep(version: version, install: install, reason: "The connected processors could not be checked, so the update waits.")
                return
            }
            if active.isEmpty { self.note("Installing \(version)."); install(); return }
            self.ask(version: version, processors: active, install: install)
        }
    }

    private func ask(version: String, processors: [UpdateGate.Processor], install: @escaping () -> Void) {
        asking = true
        let text = UpdateGate.message(version: version, processors: processors)
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = text.title
        alert.informativeText = text.detail
        alert.addButton(withTitle: "Disconnect and Install")
        alert.addButton(withTitle: "Later")
        NSApp.activate(ignoringOtherApps: true)
        let answer = alert.runModal()
        asking = false
        guard answer == .alertFirstButtonReturn else {
            keep(version: version, install: install, reason: "Update \(version) is ready; it installs when you quit or choose Install Update….")
            return
        }
        disconnectAll { [weak self] idle in
            guard let self else { return }
            if idle { self.note("Disconnected; installing \(version)."); install() }
            else { self.keep(version: version, install: install, reason: "A processor is still connected, so update \(version) waits.") }
        }
    }

    private func keep(version: String, install: @escaping () -> Void, reason: String) {
        pendingVersion = version
        pendingInstall = install
        note(reason)
    }

    /// Every session ends as Disconnect ends it; then the service is asked
    /// until no session is left (at most 15 s).
    private func disconnectAll(_ completion: @escaping (Bool) -> Void) {
        var request = URLRequest(url: BackendHealth.api("disconnect-all"))
        request.httpMethod = "POST"
        request.setValue(BackendHealth.origin, forHTTPHeaderField: "Origin")
        urlSession.dataTask(with: request) { [weak self] _, _, _ in
            DispatchQueue.main.async { self?.waitForIdle(attempt: 0, completion) }
        }.resume()
    }

    private func waitForIdle(attempt: Int, _ completion: @escaping (Bool) -> Void) {
        activeProcessors { [weak self] active in
            if active?.isEmpty == true { completion(true); return }
            if attempt >= 30 { completion(false); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self?.waitForIdle(attempt: attempt + 1, completion) }
        }
    }

    // MARK: SPUUpdaterDelegate

    func feedURLString(for updater: SPUUpdater) -> String? {
        UpdateGate.feedOverride()?.absoluteString
    }

    /// Install and Relaunch was chosen: Sparkle waits until `block` runs.
    func updater(_ updater: SPUUpdater, shouldPostponeRelaunchForUpdate item: SUAppcastItem, untilInvokingBlock block: @escaping () -> Void) -> Bool {
        let version = item.displayVersionString
        DispatchQueue.main.async { [weak self] in self?.installWhenIdle(version: version, install: block) }
        return true
    }

    /// A downloaded update installs when the app quits (Quit ends every session cleanly);
    /// Install Update… can install it sooner.
    func updater(_ updater: SPUUpdater, willInstallUpdateOnQuit item: SUAppcastItem, immediateInstallationBlock immediateInstallHandler: @escaping () -> Void) -> Bool {
        keep(version: item.displayVersionString, install: immediateInstallHandler,
             reason: "Update \(item.displayVersionString) is ready; it installs when you quit or choose Install Update….")
        return true
    }

    func updater(_ updater: SPUUpdater, didFindValidUpdate item: SUAppcastItem) {
        note("Update \(item.displayVersionString) is available.")
    }

    func updaterDidNotFindUpdate(_ updater: SPUUpdater, error: any Error) {
        note("Up to date.")
    }

    func updater(_ updater: SPUUpdater, failedToDownloadUpdate item: SUAppcastItem, error: any Error) {
        note("Update \(item.displayVersionString) could not be downloaded: \(error.localizedDescription)")
    }

    func updater(_ updater: SPUUpdater, didAbortWithError error: any Error) {
        let code = (error as NSError).code
        // "No update" also ends a cycle with an error in Sparkle; it is not a failure.
        if code == Int(SUError.noUpdateError.rawValue) { note("Up to date."); return }
        note("Update failed: \(error.localizedDescription)")
    }
}
