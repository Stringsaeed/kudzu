import Foundation

/// A fresh, empty temp folder (short path: Unix socket paths are limited to 104 bytes).
func tempDir(_ name: String) -> URL {
    let url = URL(fileURLWithPath: "/tmp", isDirectory: true)
        .appendingPathComponent("kz-\(name)-\(UUID().uuidString.prefix(8))", isDirectory: true)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

func read(_ url: URL) -> String? { try? String(contentsOf: url, encoding: .utf8) }

func write(_ text: String, to url: URL) {
    try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try? Data(text.utf8).write(to: url)
}

extension Array where Element == UInt8 {
    var hex: String { map { String(format: "%02x", $0) }.joined() }
}
