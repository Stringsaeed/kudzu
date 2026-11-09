import Darwin
import Foundation
import Security

/// Swaps a downloaded `.app.tar.gz` into place and relaunches (what tauri-plugin-updater's
/// macOS `install` and `app.restart()` did).
///
/// The archive is unpacked next to the running bundle (same volume), the new bundle replaces the
/// old one with `FileManager.replaceItemAt` (an atomic swap), and the old copy is kept beside it
/// as ``previousName(for:)`` until the new version has run for a while
/// (``removePreviousVersion(beside:)``), so a build that can't start can be put back by hand.
/// A failure leaves the running app untouched.
public enum Installer {
    /// Largest archive the updater downloads (a release is ~30 MB).
    public static let maxArchiveBytes: Int64 = 512 << 20

    /// The hidden copy of the replaced bundle: `.Kudzu-previous.app` next to `Kudzu.app`.
    public static func previousName(for bundle: URL) -> String {
        ".\(bundle.deletingPathExtension().lastPathComponent)-previous.app"
    }

    /// Removes the copy of the version an update replaced (call once the new version runs).
    public static func removePreviousVersion(beside bundle: URL) {
        let previous = bundle.deletingLastPathComponent().appendingPathComponent(previousName(for: bundle), isDirectory: true)
        guard FileManager.default.fileExists(atPath: previous.path) else { return }
        try? FileManager.default.removeItem(at: previous)
    }

    /// The `.app` folder this process runs from, or `nil` for a bare executable.
    public static var bundleURL: URL? {
        let url = Bundle.main.bundleURL
        return url.pathExtension.lowercased() == "app" ? url : nil
    }

    /// Kudzu can only replace itself when it runs from a writable, non-quarantined location.
    /// Running straight from the DMG, or from App Translocation's randomized read-only copy
    /// (a quarantined app that was never moved), makes installs fail.
    public static func locationProblem(bundle: URL? = bundleURL) -> String? {
        let path = (bundle ?? Bundle.main.executableURL ?? Bundle.main.bundleURL).path
        if path.contains("/AppTranslocation/") || path.hasPrefix("/Volumes/") {
            return "Move Kudzu to your Applications folder so it can update itself."
        }
        // The .app bundle's parent folder must be writable.
        guard let bundle else { return nil }
        let probe = bundle.deletingLastPathComponent().appendingPathComponent(".kudzu-write-test-\(UUID().uuidString)")
        let fd = Darwin.open(probe.path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0o600)
        if fd >= 0 {
            Darwin.close(fd)
            unlink(probe.path)
            return nil
        }
        return "Kudzu can't write to the folder it's in. Move it to Applications to keep it updated."
    }

    /// Unpacks `archive` (a gzipped tar holding one `.app`) and swaps it in for `bundle`.
    ///
    /// The minisign signature covers the archive bytes but not the feed's `version`, so the
    /// unpacked bundle's own version must be the one the feed announced (`expectedVersion`) and
    /// newer than `currentVersion`: a tampered feed can't pass off an older signed release.
    public static func install(archive: Data, over bundle: URL, expectedVersion: String? = nil, currentVersion: String? = nil) throws {
        let fm = FileManager.default
        let work = try fm.url(for: .itemReplacementDirectory, in: .userDomainMask, appropriateFor: bundle, create: true)
        defer { try? fm.removeItem(at: work) }
        let tarball = work.appendingPathComponent("update.tar.gz")
        try archive.write(to: tarball)
        let unpacked = work.appendingPathComponent("unpacked", isDirectory: true)
        try fm.createDirectory(at: unpacked, withIntermediateDirectories: true)
        try run("/usr/bin/tar", ["-xzf", tarball.path, "-C", unpacked.path])
        guard let app = try fm.contentsOfDirectory(at: unpacked, includingPropertiesForKeys: nil)
            .first(where: { $0.pathExtension.lowercased() == "app" }) else {
            throw ServiceError("The update archive doesn't contain an app.")
        }
        // Minisign authenticates the archive. Also require the new bundle to satisfy this
        // installation's designated requirement (identifier and signer), before changing either
        // the installed app or the candidate's quarantine metadata.
        try checkCandidate(app, expectedVersion: expectedVersion, currentVersion: currentVersion)
        try validateBundle(app, matching: bundle)
        // Downloaded in memory, so no quarantine flag; clear one anyway in case tar carried it.
        _ = try? run("/usr/bin/xattr", ["-dr", "com.apple.quarantine", app.path])
        removePreviousVersion(beside: bundle)
        _ = try fm.replaceItemAt(bundle, withItemAt: app, backupItemName: previousName(for: bundle), options: [.withoutDeletingBackupItem])
    }

    /// What the candidate's `Info.plist` says, before its signature is checked: it must be the
    /// native app (not a Tauri release, which shares the feed, bundle id and signer), carry the
    /// version the feed announced, and be newer than this one (never a downgrade).
    static func checkCandidate(_ candidate: URL, expectedVersion: String?, currentVersion: String?) throws {
        let info = candidate.appendingPathComponent("Contents/Info.plist")
        guard let data = try? Data(contentsOf: info),
              let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] else {
            throw ServiceError("The update isn't an application bundle.")
        }
        if let executable = plist["CFBundleExecutable"] as? String, executable == "kudzu-desktop" {
            throw ServiceError("The update is for an earlier edition of Kudzu. It was not installed.")
        }
        let found = (plist["CFBundleShortVersionString"] as? String).flatMap(SemanticVersion.init)
        if let expectedVersion {
            guard let found, let expected = SemanticVersion(expectedVersion), found == expected else {
                throw ServiceError("The update's version doesn't match the update feed. It was not installed.")
            }
        }
        if let currentVersion, let current = SemanticVersion(currentVersion) {
            guard let found, found > current else {
                throw ServiceError("The update isn't newer than this version of Kudzu. It was not installed.")
            }
        }
    }

    static func validateBundle(_ candidate: URL, matching installed: URL) throws {
        let values = try candidate.resourceValues(forKeys: [.isSymbolicLinkKey, .isDirectoryKey])
        guard values.isSymbolicLink != true, values.isDirectory == true else {
            throw ServiceError("The update isn't an application bundle.")
        }
        var previous: SecStaticCode?
        var next: SecStaticCode?
        var requirement: SecRequirement?
        let flags = SecCSFlags(rawValue: kSecCSCheckAllArchitectures | kSecCSStrictValidate)
        guard SecStaticCodeCreateWithPath(installed as CFURL, [], &previous) == errSecSuccess,
              let previous,
              SecCodeCopyDesignatedRequirement(previous, [], &requirement) == errSecSuccess,
              let requirement,
              SecStaticCodeCreateWithPath(candidate as CFURL, [], &next) == errSecSuccess,
              let next,
              SecStaticCodeCheckValidity(next, flags, requirement) == errSecSuccess else {
            throw ServiceError("The update's code signature doesn't match this installation. It was not installed.")
        }
    }

    /// Opens `bundle` again once this process (`pid`) has exited, so the single-instance check
    /// sees no running copy. The helper is a detached `/bin/sh` that outlives us; it gives up
    /// after 60 s, so a quit that never happens can't leave it running (or open a second copy).
    public static func relaunch(_ bundle: URL, after pid: Int32 = ProcessInfo.processInfo.processIdentifier) throws {
        let script = "n=0; while /bin/kill -0 \(pid) 2>/dev/null; do n=$((n+1)); [ $n -ge 600 ] && exit 0; /bin/sleep 0.1; done; /usr/bin/open \"$0\""
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/sh")
        p.arguments = ["-c", script, bundle.path]
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        try p.run()
    }

    @discardableResult
    static func run(_ tool: String, _ args: [String]) throws -> Int32 {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: tool)
        p.arguments = args
        p.standardOutput = FileHandle.nullDevice
        let err = Pipe()
        p.standardError = err
        try p.run()
        // Drain before waiting: a tar/codesign error can fill the pipe and block the child.
        let errorData = err.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        guard p.terminationStatus == 0 else {
            let message = String(decoding: errorData, as: UTF8.self)
            throw ServiceError("\(URL(fileURLWithPath: tool).lastPathComponent) failed: \(message.trimmingCharacters(in: .whitespacesAndNewlines))")
        }
        return p.terminationStatus
    }
}
