// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "welang",
    targets: [
        .systemLibrary(
            name: "CLLLVM",
            pkgConfig: "llvm-18",
            providers: [
                .apt(["llvm-18-dev"]),
            ]
        ),
        .target(
            name: "WeLangLib",
            dependencies: ["CLLLVM"],
            path: "Sources/WeLangLib"
        ),
        .executableTarget(
            name: "welang",
            dependencies: ["WeLangLib"],
            path: "Sources/WeLang"
        ),
        .testTarget(
            name: "WeLangTests",
            dependencies: ["WeLangLib"],
            path: "Tests/WeLangTests"
        ),
    ]
)
