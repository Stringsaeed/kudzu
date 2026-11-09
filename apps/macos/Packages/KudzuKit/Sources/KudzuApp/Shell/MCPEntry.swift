import Foundation
import KudzuServices

/// `Kudzu --mcp [--dev]`: the stdio bridge an MCP client launches. It relays JSON-RPC lines
/// between stdin/stdout and the running app's `agent.sock` (the dev variant's with `--dev`), and
/// answers offline (tools listed, calls explaining what to turn on) when the app isn't reachable.
/// Runs before AppKit starts.
///
/// - Returns: the process exit status.
public func runMCPBridge(dev: Bool) -> Int32 {
    MCPBridge.run(dev: dev)
}
