import AppKit

@main
enum PresetsWindowTests {
    @MainActor
    static func main() {
        _ = NSApplication.shared
        let controller = PresetsWindowController(deviceID: "unit-a")
        guard let window = controller.window else { preconditionFailure("Presets window missing") }
        precondition(window.title == "Presets")
        guard let splitController = window.contentViewController as? NSSplitViewController else {
            preconditionFailure("Presets content must use a native split view controller")
        }
        guard let sidebarItem = splitController.splitViewItems.first else {
            preconditionFailure("Presets sidebar item missing")
        }
        precondition(sidebarItem.behavior == .sidebar)
        precondition(sidebarItem.minimumThickness == PresetPresentation.minimumSidebarWidth)
        precondition(sidebarItem.maximumThickness == PresetPresentation.maximumSidebarWidth)
        precondition(window.styleMask.contains(.fullSizeContentView))
        let contentSize = window.contentRect(forFrameRect: window.frame).size
        precondition(contentSize.width >= PresetPresentation.minimumSize.width)
        precondition(contentSize.height >= PresetPresentation.minimumSize.height)
        precondition(window.contentMinSize == PresetPresentation.minimumSize)
        // Each processor keeps its own Presets window position.
        precondition(window.frameAutosaveName == "OpenOptimod.presets.unit-a")
        let labels = window.toolbar?.items.map(\.label) ?? []
        for label in ["Recall", "Save", "Save As…", "Preset Actions", "Search"] {
            precondition(labels.contains(label), "toolbar lacks \(label)")
        }
        let tables = descendants(of: NSTableView.self, in: splitController.splitView)
        // Double-click recalls (after asking), and every row has a context menu.
        precondition(tables.contains { $0.rowHeight == PresetPresentation.rowHeight && $0.doubleAction != nil && $0.menu != nil })
        let state = controller.testState
        precondition(state["kind"] as? String == "native-presets")
        precondition((state["enabled"] as? [String: Bool])?["recall"] == false, "nothing selected, nothing to recall")
        print("PresetsWindowTests passed")
    }

    private static func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
        root.subviews.flatMap { view -> [T] in
            let current = (view as? T).map { [$0] } ?? []
            return current + descendants(of: type, in: view)
        }
    }
}
