import Foundation

extension AppModel {
    /// Feature-owned state survives removing and recreating the editor pane. Weak keys release it
    /// with the window model; no process-global draft is shared between windows.
    public var editorSession: EditorSession {
        if let session = EditorSessions.values.object(forKey: self) { return session }
        let session = EditorSession(model: self)
        EditorSessions.values.setObject(session, forKey: self)
        return session
    }

    /// T12 can await this before window-level commands that consume the selected snippet.
    @discardableResult
    public func flushEditor() async -> Bool { await editorSession.flush() }
}

@MainActor private enum EditorSessions {
    static let values = NSMapTable<AppModel, EditorSession>.weakToStrongObjects()
}
