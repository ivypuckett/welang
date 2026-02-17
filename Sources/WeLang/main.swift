import Foundation
import WeLangLib

// MARK: - CLI Entry Point

let args = CommandLine.arguments

guard args.count >= 2 else {
    fputs("Usage: welang <source-file>\n", stderr)
    exit(1)
}

let filename = args[1]

let source: String
do {
    source = try String(contentsOfFile: filename, encoding: .utf8)
} catch {
    fputs("Error reading file '\(filename)': \(error)\n", stderr)
    exit(1)
}

do {
    try compile(source)
} catch {
    fputs("Compilation error: \(error)\n", stderr)
    exit(1)
}
