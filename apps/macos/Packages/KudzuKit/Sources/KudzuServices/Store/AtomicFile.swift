import Darwin
import Foundation

/// `store::write_atomic`: every write goes to a temp file first, is flushed to disk (`fsync`)
/// and renamed into place, so a crash never leaves a half-written library.
public enum AtomicFile {
    /// Base name next to `url`: `library.json` → `library.tmp`, followed by a per-write suffix.
    public static func tempURL(for url: URL) -> URL {
        url.deletingPathExtension().appendingPathExtension("tmp")
    }

    /// Writes `data` to `url` atomically. Each caller owns a separate temp file, so overlapping
    /// exports or saves cannot truncate or rename another writer's in-flight payload.
    public static func write(_ data: Data, to url: URL, mode: mode_t = 0o644) throws {
        let tmp = tempURL(for: url).appendingPathExtension(UUID().uuidString)
        let fd = Darwin.open(tmp.path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, mode)
        guard fd >= 0 else { throw posixError("open", tmp) }
        var failure: Error?
        data.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            var offset = 0
            while offset < raw.count {
                let n = Darwin.write(fd, base.advanced(by: offset), raw.count - offset)
                if n < 0 {
                    if errno == EINTR { continue }
                    failure = posixError("write", tmp)
                    return
                }
                offset += n
            }
        }
        if failure == nil, fsync(fd) != 0 { failure = posixError("fsync", tmp) }
        close(fd)
        if let failure {
            unlink(tmp.path)
            throw failure
        }
        guard rename(tmp.path, url.path) == 0 else {
            let error = posixError("rename", url)
            unlink(tmp.path)
            throw error
        }
    }

    static func posixError(_ op: String, _ url: URL) -> ServiceError {
        ServiceError("\(op) \(url.lastPathComponent): \(String(cString: strerror(errno)))")
    }
}
