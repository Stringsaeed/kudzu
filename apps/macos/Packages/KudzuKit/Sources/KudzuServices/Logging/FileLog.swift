import Foundation

/// The rotating `Kudzu.log`: appended on a serial queue, rotated at 2 MB, three files kept
/// (`Kudzu.log`, `Kudzu.1.log`, `Kudzu.2.log`). Line format follows tauri-plugin-log:
/// `[2026-11-08][14:03:21][category][INFO] message`, in UTC.
final class FileLog: @unchecked Sendable {
    // @unchecked: `handle`, `url` and `size` are only touched on `queue`.
    static let shared = FileLog()

    static let maxBytes: UInt64 = 2_000_000
    static let keep = 3
    static let fileName = "Kudzu"

    private let queue = DispatchQueue(label: "sh.saeed.kudzu.log", qos: .utility)
    private var directory: URL?
    private var handle: FileHandle?
    private var size: UInt64 = 0

    private let formatter: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC") // tauri-plugin-log's default
        f.dateFormat = "'['yyyy-MM-dd']['HH:mm:ss']'"
        return f
    }()

    func open(directory: URL) {
        queue.async {
            try? self.handle?.close()
            self.handle = nil
            self.directory = directory
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            self.reopen()
        }
    }

    func write(level: String, category: String, message: String) {
        let now = Date()
        queue.async {
            guard self.handle != nil else { return }
            let stamp = self.formatter.string(from: now)
            let line = "\(stamp)[\(category)][\(level)] \(message)\n"
            let data = Data(line.utf8)
            if self.size + UInt64(data.count) > Self.maxBytes { self.rotate() }
            self.handle?.write(data)
            self.size += UInt64(data.count)
        }
    }

    /// Waits for queued lines (tests, quit).
    func flush() { queue.sync { try? handle?.synchronize() } }

    private func path(_ index: Int) -> URL? {
        directory?.appendingPathComponent(index == 0 ? "\(Self.fileName).log" : "\(Self.fileName).\(index).log")
    }

    private func reopen() {
        guard let url = path(0) else { return }
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
        handle = try? FileHandle(forWritingTo: url)
        size = (try? handle?.seekToEnd()) ?? 0
    }

    private func rotate() {
        try? handle?.close()
        handle = nil
        let fm = FileManager.default
        for i in stride(from: Self.keep - 1, through: 1, by: -1) {
            guard let from = path(i - 1), let to = path(i) else { continue }
            try? fm.removeItem(at: to)
            try? fm.moveItem(at: from, to: to)
        }
        reopen()
    }
}
