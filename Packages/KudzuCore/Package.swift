// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "KudzuCore",
    platforms: [.macOS(.v15), .iOS(.v18)],
    products: [
        .library(name: "KudzuCore", targets: ["KudzuCore"]),
    ],
    dependencies: [
        .package(url: "https://github.com/jpsim/Yams.git", from: "5.1.0"),
    ],
    targets: [
        .target(
            name: "KudzuCore",
            dependencies: ["Yams"]
        ),
        .testTarget(
            name: "KudzuCoreTests",
            dependencies: ["KudzuCore"],
            resources: [.copy("Fixtures")]
        ),
    ]
)
