// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AppleFMBridge",
    // Minimum macOS 15 so host binaries still run on machines without the
    // FoundationModels framework; every use of the framework is guarded
    // with `#available(macOS 26.0, *)` and the framework is weak-linked
    // by the crate's build script.
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "AppleFMBridge", type: .static, targets: ["AppleFMBridge"])
    ],
    targets: [
        .target(name: "AppleFMBridge")
    ]
)
