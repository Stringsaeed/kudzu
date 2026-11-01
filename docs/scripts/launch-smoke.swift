import AppKit
import Foundation

guard CommandLine.arguments.count == 2 else {
    fatalError("Usage: swift docs/scripts/launch-smoke.swift <build-products-directory>")
}
let root = URL(fileURLWithPath: CommandLine.arguments[1])
for name in ["Kudzu", "KudzuAppStore"] {
    let app = root.appendingPathComponent("\(name).app")
    let executable = app.appendingPathComponent("Contents/MacOS/\(name)").path
    guard !NSWorkspace.shared.runningApplications.contains(where: { $0.executableURL?.path == executable }) else {
        fatalError("Quit the existing \(name) instance before the smoke test")
    }
    let launch = Process()
    launch.executableURL = URL(fileURLWithPath: "/usr/bin/open")
    launch.arguments = [app.path]
    try launch.run()
    launch.waitUntilExit()
    guard launch.terminationStatus == 0 else { fatalError("open failed: \(name)") }
    RunLoop.current.run(until: Date().addingTimeInterval(3))
    let applications = NSWorkspace.shared.runningApplications.filter { $0.executableURL?.path == executable }
    guard applications.count == 1, let running = applications.first else { fatalError("Not running: \(name)") }
    let pid = running.processIdentifier
    print("\(name): alive after 3s, PID \(pid)")
    guard running.terminate() else { fatalError("Quit request failed: \(name)") }
    RunLoop.current.run(until: Date().addingTimeInterval(3))
    guard NSRunningApplication(processIdentifier: pid) == nil else { fatalError("Still running: \(name)") }
    print("\(name): quit confirmed")
}
