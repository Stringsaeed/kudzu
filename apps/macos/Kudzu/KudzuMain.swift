import AppKit
import KudzuApp

/// The process entry point. `Kudzu --mcp [--dev]` is the stdio bridge an MCP client launches: it
/// runs before AppKit starts and exits with its status. Otherwise this starts the menu-bar app;
/// `--hidden` (the login item) starts it without opening the main window.
@main
@MainActor
enum KudzuMain {
    static func main() {
        let arguments = CommandLine.arguments.dropFirst()
        if arguments.contains("--mcp") {
            exit(KudzuApp.runMCPBridge(dev: arguments.contains("--dev")))
        }
        let app = NSApplication.shared
        let delegate = AppDelegate(launchedHidden: arguments.contains("--hidden"))
        app.delegate = delegate
        // `NSApplication.delegate` is weak: keep ours alive for the life of the run loop.
        withExtendedLifetime(delegate) { app.run() }
    }
}
