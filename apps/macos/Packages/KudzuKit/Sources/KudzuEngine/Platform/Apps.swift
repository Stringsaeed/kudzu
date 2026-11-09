import AppKit
import Foundation
import KudzuCore

// Running and installed applications, activation and icons. Port of `macos/apps.rs`.

enum Apps {
    private static func info(_ app: NSRunningApplication) -> AppInfo {
        let path = app.bundleURL?.path
        let bundleId = app.bundleIdentifier ?? ""
        let name = app.localizedName ?? path.map(displayName(fromPath:)) ?? bundleId
        return AppInfo(bundleId: bundleId, name: name, pid: app.processIdentifier, path: path)
    }

    /// The file stem of `path` ("/Applications/Notes.app" → "Notes").
    static func displayName(fromPath path: String) -> String {
        URL(fileURLWithPath: path).deletingPathExtension().lastPathComponent
    }

    /// The frontmost application.
    static func frontmostApp() -> AppInfo? {
        autoreleasepool { NSWorkspace.shared.frontmostApplication.map(info) }
    }

    /// Pid of the frontmost application (cheap; safe from the tap thread).
    static func frontmostPid() -> pid_t? {
        autoreleasepool { NSWorkspace.shared.frontmostApplication?.processIdentifier }
    }

    /// Bundle id of the running process `pid`.
    static func bundleId(of pid: pid_t) -> String? {
        autoreleasepool { NSRunningApplication(processIdentifier: pid)?.bundleIdentifier }
    }

    /// Localized name of the running process `pid`.
    static func name(of pid: pid_t) -> String? {
        autoreleasepool { NSRunningApplication(processIdentifier: pid)?.localizedName }
    }

    private static func running(_ app: AppInfo) -> NSRunningApplication? {
        if let pid = app.pid, let found = NSRunningApplication(processIdentifier: pid) {
            return found
        }
        if app.bundleId.isEmpty {
            return nil
        }
        return NSRunningApplication.runningApplications(withBundleIdentifier: app.bundleId).first
    }

    /// Brings `app` to the front (by pid, else bundle id). Returns whether the request was accepted.
    @discardableResult
    static func activate(_ app: AppInfo) -> Bool {
        autoreleasepool { running(app)?.activate(options: []) ?? false }
    }

    /// Pid `app` resolves to when running.
    static func resolvePid(_ app: AppInfo) -> pid_t? {
        autoreleasepool { running(app)?.processIdentifier }
    }

    /// Activates `app` and waits (≤ `timeout`) until it is frontmost.
    static func activateAndWait(_ app: AppInfo, timeout: TimeInterval) -> Bool {
        guard let pid = resolvePid(app) else { return false }
        if frontmostPid() == pid {
            return true
        }
        activate(app)
        let deadline = RuntimeTiming.now() + timeout
        while RuntimeTiming.now() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
            if frontmostPid() == pid {
                return true
            }
        }
        return false
    }

    /// Opens a URL with its default handler.
    static func openURL(_ string: String) -> Bool {
        autoreleasepool {
            guard let url = URL(string: string) else { return false }
            return NSWorkspace.shared.open(url)
        }
    }

    // MARK: - Installed apps

    static func applicationRoots() -> [URL] {
        var roots = ["/Applications", "/Applications/Utilities", "/System/Applications", "/System/Applications/Utilities"]
            .map { URL(fileURLWithPath: $0, isDirectory: true) }
        if let home = ProcessInfo.processInfo.environment["HOME"] {
            roots.append(URL(fileURLWithPath: home, isDirectory: true).appendingPathComponent("Applications", isDirectory: true))
        }
        return roots
    }

    private static func isApp(_ url: URL) -> Bool {
        url.pathExtension.caseInsensitiveCompare("app") == .orderedSame
    }

    private static func isDirectory(_ url: URL) -> Bool {
        var directory: ObjCBool = false
        // `fileExists` follows symlinks, like Rust's `Path::is_dir`.
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && directory.boolValue
    }

    private static func children(_ url: URL) -> [URL]? {
        try? FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: nil, options: [])
    }

    /// `.app` bundles directly in `root` and in its plain sub-folders (one level deep).
    static func appBundles(_ root: URL) -> [URL] {
        var found: [URL] = []
        for path in children(root) ?? [] {
            if isApp(path) {
                found.append(path)
            } else if isDirectory(path), let nested = children(path) {
                found.append(contentsOf: nested.filter(isApp))
            }
        }
        return found
    }

    static func readBundle(_ path: URL) -> AppInfo? {
        let plistURL = path.appendingPathComponent("Contents/Info.plist")
        guard let data = try? Data(contentsOf: plistURL),
              let plist = try? PropertyListSerialization.propertyList(from: data, format: nil),
              let dict = plist as? [String: Any]
        else { return nil }
        func string(_ key: String) -> String? {
            guard let value = (dict[key] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else { return nil }
            return value
        }
        guard let bundleId = string("CFBundleIdentifier") else { return nil }
        let name = string("CFBundleDisplayName") ?? string("CFBundleName") ?? displayName(fromPath: path.path)
        return AppInfo(bundleId: bundleId, name: name, pid: nil, path: path.path)
    }

    /// Installed apps, deduped by bundle id (first root wins), sorted by name.
    static func installedApps() -> [AppInfo] {
        var byId: [String: AppInfo] = [:]
        var seenPaths = Set<String>()
        for root in applicationRoots() {
            for bundle in appBundles(root) {
                let key = bundle.resolvingSymlinksInPath().standardizedFileURL.path
                guard seenPaths.insert(key).inserted else { continue }
                if let app = readBundle(bundle), byId[app.bundleId] == nil {
                    byId[app.bundleId] = app
                }
            }
        }
        return byId.values.sorted(by: orderedBefore)
    }

    /// Byte-wise comparisons, like Rust's `str::cmp`.
    static func orderedBefore(_ a: AppInfo, _ b: AppInfo) -> Bool {
        func less(_ a: String, _ b: String) -> Bool { a.utf8.lexicographicallyPrecedes(b.utf8) }
        let (x, y) = (a.name.lowercased(), b.name.lowercased())
        return x.utf8.elementsEqual(y.utf8) ? less(a.bundleId, b.bundleId) : less(x, y)
    }

    // MARK: - Icons

    /// PNG of the icon for `path`, drawn at exactly `size`×`size` pixels.
    static func iconPNG(path: String, size: Int) -> Data? {
        guard size > 0, size <= 1024, FileManager.default.fileExists(atPath: path) else { return nil }
        return autoreleasepool {
            let image = NSWorkspace.shared.icon(forFile: path)
            return drawPNG(image, width: size, height: size)
        }
    }

    /// Draws `image` into a `width`×`height` RGBA bitmap and encodes it as PNG.
    /// Safe off the main thread (it uses its own bitmap context).
    static func drawPNG(_ image: NSImage, width: Int, height: Int) -> Data? {
        guard width > 0, height > 0, width <= 8192, height <= 8192 else { return nil }
        guard let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: width,
            pixelsHigh: height,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 32
        ) else { return nil }
        let points = NSSize(width: width, height: height)
        rep.size = points
        guard let context = NSGraphicsContext(bitmapImageRep: rep) else { return nil }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        context.imageInterpolation = .high
        image.draw(in: NSRect(origin: .zero, size: points), from: .zero, operation: .copy, fraction: 1.0)
        context.flushGraphics()
        NSGraphicsContext.restoreGraphicsState()
        return rep.representation(using: .png, properties: [:])
    }
}
