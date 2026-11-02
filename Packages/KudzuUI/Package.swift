// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "KudzuUI",
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "KudzuUI", targets: ["KudzuUI"]),
    ],
    targets: [
        .target(
            name: "KudzuUI",
            resources: [
                // Copied (not processed) so the folder layout survives into the bundle.
                .copy("Resources/Fonts"),
                .copy("Resources/MenuBar"),
            ]
        ),
        .testTarget(
            name: "KudzuUITests",
            dependencies: ["KudzuUI"]
        ),
    ],
    swiftLanguageModes: [.v6]
)
