/// A complete program: the root of the AST.
public struct Program: Equatable {
    public var items: [Item]

    public init(items: [Item]) {
        self.items = items
    }
}

/// A top-level item in the program.
///
/// This enum will be extended with function definitions, type declarations,
/// etc. as the language grows.
public enum Item: Equatable {
    /// Placeholder — will be replaced by real language constructs.
    case placeholder(Span)
}
