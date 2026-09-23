import AppKit

@main
enum ConnectionsWindowTests {
    @MainActor
    static func main() {
        _ = NSApplication.shared
        let controller = ConnectionsWindowController()
        guard let window = controller.window else { preconditionFailure("Connections window missing") }

        let contentSize = window.contentRect(forFrameRect: window.frame).size
        precondition(ConnectionsPresentation.defaultSize == NSSize(width: 920, height: 500))
        precondition(contentSize.width >= ConnectionsPresentation.minimumSize.width)
        precondition(contentSize.height >= ConnectionsPresentation.minimumSize.height)
        precondition(window.contentMinSize == ConnectionsPresentation.minimumSize)
        precondition(window.styleMask.contains(.fullSizeContentView))
        precondition(window.subtitle.isEmpty)

        guard let splitController = window.contentViewController as? NSSplitViewController else {
            preconditionFailure("Connections content must use a native split view controller")
        }
        guard let sidebarItem = splitController.splitViewItems.first else {
            preconditionFailure("Connections sidebar item missing")
        }
        precondition(sidebarItem.behavior == .sidebar)
        precondition(sidebarItem.minimumThickness == ConnectionsPresentation.minimumSidebarWidth)
        precondition(sidebarItem.maximumThickness == ConnectionsPresentation.maximumSidebarWidth)
        let split = splitController.splitView
        split.layoutSubtreeIfNeeded()
        precondition((ConnectionsPresentation.minimumSidebarWidth ... ConnectionsPresentation.maximumSidebarWidth).contains(split.arrangedSubviews.first?.frame.width ?? 0))

        let tables = descendants(of: NSTableView.self, in: split)
        precondition(tables.contains { $0.rowHeight == ConnectionsPresentation.rowHeight })
        precondition(window.toolbar?.items.contains { $0.itemIdentifier == .toggleSidebar } == true)
        print("ConnectionsWindowTests passed")
    }

    private static func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
        root.subviews.flatMap { view -> [T] in
            let current = (view as? T).map { [$0] } ?? []
            return current + descendants(of: type, in: view)
        }
    }
}
