/// Top-level compilation pipeline: lex -> parse -> codegen.
///
/// This is the library-level entry point for the compiler. It runs all
/// phases in sequence and propagates any errors.
public func compile(_ source: String) throws {
    let tokens = try lex(source)
    let ast = try parse(tokens)
    try generate(ast)
}
