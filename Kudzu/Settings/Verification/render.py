#!/usr/bin/env python3
"""Render the actual settings views with a temporary, inert AppModel composition root.
Run after building Kudzu. No app launch, event tap, hotkey, scheduler or real store.
"""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[3]
products = root / 'build/DerivedData/Build/Products/Debug'
output = root / 'build/settings-snapshots'
output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='kudzu-settings-render-') as directory:
    temp = Path(directory)
    sources = []
    for source in sorted((root / 'Kudzu').rglob('*.swift')):
        if source.name == 'KudzuApp.swift':
            continue
        text = source.read_text()
        if source.name == 'AppModel.swift':
            assert text.count('store = try KudzuStore(syncEnabled: false)') == 1, 'Store initialization changed; audit the inert renderer.'
            text = text.replace('store = try KudzuStore(syncEnabled: false)', 'store = try KudzuStore.inMemory()')
            text = text.replace('BackupScheduler.defaultBackupDirectory()', 'URL(fileURLWithPath: "' + directory + '/backups")')
            for call in ['engine.start()', 'backupScheduler.start()', 'startPalette()']:
                assert text.count('        ' + call) == 1, 'Service startup changed; audit the inert renderer.'
                text = text.replace('        ' + call, '        // Disabled in the isolated offscreen renderer.')
        if source.name == 'AppCatalog.swift':
            text = text.replace('let info: Info', 'var info: Info')
            text = text.replace('        cache[bundleID] = info', """
        // Flatten wide-gamut AppKit icons before ImageRenderer chooses its output color space.
        if let image = info.icon.cgImage(forProposedRect: nil, context: nil, hints: nil),
           let context = CGContext(data: nil, width: image.width, height: image.height,
               bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
               bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) {
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            if let flattened = context.makeImage() { info.icon = NSImage(cgImage: flattened, size: info.icon.size) }
        }
        cache[bundleID] = info""")
        if source.name == 'AppsSettingsTab.swift':
            text = text.replace('AppMatchingSettings = .shared',
                                'AppMatchingSettings = AppMatchingSettings(defaults: UserDefaults(suiteName: "settings-snapshot-" + UUID().uuidString)!)')
        target = temp / source.name
        target.write_text(text)
        sources.append(str(target))
    runner = temp / 'RenderSettings.swift'
    runner.write_text('''
import AppKit
import SwiftUI
import KudzuCore
import KudzuUI
@main struct RenderSettings {
    @MainActor static func write<V: View>(_ view: V, named name: String, to output: URL, width: CGFloat) throws {
        for appearance in KzSnapshot.Appearance.allCases where appearance.rawValue == CommandLine.arguments[3] {
            NSApp.appearance = appearance.nsAppearance
            guard let image = KzSnapshot.image(of: view.transaction { $0.disablesAnimations = true }, appearance: appearance, width: width),
                  let context = CGContext(data: nil, width: image.width, height: image.height,
                      bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { fatalError("Snapshot failed") }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            guard let normalized = context.makeImage(), let data = KzSnapshot.pngData(normalized) else { fatalError("PNG failed") }
            try data.write(to: output.appendingPathComponent("\\(name)-\\(appearance.rawValue)@2x.png"))
        }
    }
    @MainActor static func main() throws {
        _ = NSApplication.shared
        NSApp.appearance = CommandLine.arguments[3] == "dark" ? NSAppearance(named: .darkAqua) : NSAppearance(named: .aqua)
        KzFonts.register()
        let model = AppModel()
        // NSWorkspace icon loading can change AppKit drawing appearance. Resolve before rendering.
        for id in MatchingConfig.defaultDeniedBundleIDs.union(Set(MatchingConfig.defaultAppInsertions.keys)) {
            _ = AppCatalog.info(for: id)
        }
        let output = URL(fileURLWithPath: CommandLine.arguments[1])
        for section in SettingsSection.allCases where section.rawValue == CommandLine.arguments[2] {
            try write(SettingsPage(section: section).environment(model).frame(width: 760, height: section == .apps ? 1400 : 980, alignment: .top),
                                 named: section.rawValue, to: output, width: 760)
        }
        let samples = [BackupReason.daily, .preImport, .manual].enumerated().map { index, reason in
            let name = "kudzu-backup-2026-11-02T00353\\(index)-\\(reason.rawValue).json"
            return BackupsSettingsTab.BackupRow(info: BackupInfo(url: output.appendingPathComponent(name),
                filename: name, createdAt: Date(timeIntervalSince1970: 1793579700 - Double(index * 3600)),
                reason: reason), snippetCount: 18 - index)
        }
        if CommandLine.arguments[2] == "backups-populated" {
        try write(BackupsSettingsTab(snapshotRows: samples).environment(model).frame(width: 760, height: 760),
                             named: "backups-populated", to: output, width: 760)
        }
    }
}
''')
    arch = platform.machine()
    command = ['xcrun', 'swiftc', '-parse-as-library', '-swift-version', '6', '-target', f'{arch}-apple-macos15.0',
               '-I', str(products), '-I', str(root / 'build/DerivedData/SourcePackages/checkouts/Yams/Sources/CYaml/include'), '-o', str(temp / 'render')]
    command += sources + [str(runner)]
    command += [str(products / f'{name}.o') for name in ['KudzuCore', 'KudzuUI', 'Yams', 'CYaml']]
    subprocess.run(command, cwd=root, check=True)
    shutil.copytree(products / 'KudzuUI_KudzuUI.bundle', temp / 'KudzuUI_KudzuUI.bundle')
    for page in ['general', 'apps', 'sync', 'backups', 'importExport', 'about', 'backups-populated']:
        for mode in ['light', 'dark']:
            subprocess.run([str(temp / 'render'), str(output), page, mode], cwd=root, check=True)
print(output)
