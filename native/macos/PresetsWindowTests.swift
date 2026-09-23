import AppKit

@main
enum PresetsWindowTests {
    @MainActor
    static func main() {
        _ = NSApplication.shared
        let controller = PresetsWindowController()
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
        precondition(PresetPresentation.defaultSize == NSSize(width: 900, height: 560))
        precondition(contentSize.width >= PresetPresentation.minimumSize.width)
        precondition(contentSize.height >= PresetPresentation.minimumSize.height)
        precondition(window.contentMinSize == PresetPresentation.minimumSize)
        precondition(PresetPresentation.rowHeight == 40)
        precondition(PresetPresentation.sidebarWidth == 188)
        precondition(PresetPresentation.displayName("CLASSICAL-2B+AGC") == "Classical-2B+AGC")
        precondition(PresetPresentation.displayName("COUNTRY UL") == "Country UL")
        precondition(PresetPresentation.displayName("GREGG OPEN") == "Gregg Open")
        let actionTitles = controller.window?.toolbar?.items.compactMap { ($0.view as? NSButton)?.title } ?? []
        precondition(actionTitles.contains("Recall"))
        precondition(actionTitles.contains("Apply File") == false)
        precondition(actionTitles.contains("Save to Mac") == false)
        let actionsID = NSToolbarItem.Identifier("OpenOptimodPresetActions")
        precondition(controller.window?.toolbar?.items.contains { $0.itemIdentifier == actionsID } == true)
        let tables = descendants(of: NSTableView.self, in: splitController.splitView)
        precondition(tables.contains { $0.rowHeight == PresetPresentation.rowHeight && $0.doubleAction == nil })
        print("PresetsWindowTests passed")
    }

    private static func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
        root.subviews.flatMap { view -> [T] in
            let current = (view as? T).map { [$0] } ?? []
            return current + descendants(of: type, in: view)
        }
    }
}
