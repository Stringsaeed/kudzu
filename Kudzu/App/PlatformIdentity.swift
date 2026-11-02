import Foundation
import IOKit

/// This Mac's hardware UUID, used to notice when a library file was restored onto a different machine.
enum PlatformIdentity {
    static func hardwareUUID() -> String {
        let service = IOServiceGetMatchingService(kIOMainPortDefault, IOServiceMatching("IOPlatformExpertDevice"))
        guard service != 0 else { return "" }
        defer { IOObjectRelease(service) }
        guard let property = IORegistryEntryCreateCFProperty(
            service, kIOPlatformUUIDKey as CFString, kCFAllocatorDefault, 0
        )?.takeRetainedValue() as? String else { return "" }
        return property
    }
}
