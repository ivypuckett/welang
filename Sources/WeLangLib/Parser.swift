/// Parse a token stream into an AST `Program`.
///
/// For now this simply produces an empty program so the pipeline compiles.
public func parse(_ tokens: [Token]) throws -> Program {
    // TODO: implement real parsing here.
    return Program(items: [])
}
