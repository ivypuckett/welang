import XCTest
@testable import WeLangLib

final class ASTTests: XCTestCase {

    private let s0 = Span(start: 0, end: 1)
    private let s1 = Span(start: 2, end: 3)

    // MARK: - Definition

    func testDefinitionEquality() {
        let a = Definition(label: "foo", typeAnnotation: nil, value: .integerLiteral("1", s0), span: s0)
        let b = Definition(label: "foo", typeAnnotation: nil, value: .integerLiteral("1", s0), span: s0)
        XCTAssertEqual(a, b)
    }

    func testDefinitionInequality() {
        let a = Definition(label: "foo", typeAnnotation: nil, value: .integerLiteral("1", s0), span: s0)
        let b = Definition(label: "bar", typeAnnotation: nil, value: .integerLiteral("1", s0), span: s0)
        XCTAssertNotEqual(a, b)
    }

    func testDefinitionWithTypeAnnotationEquality() {
        let ann = Expr.name("u32", s0)
        let a = Definition(label: "x", typeAnnotation: ann, value: .integerLiteral("1", s0), span: s0)
        let b = Definition(label: "x", typeAnnotation: ann, value: .integerLiteral("1", s0), span: s0)
        XCTAssertEqual(a, b)
    }

    func testDefinitionWithAndWithoutTypeAnnotation() {
        let ann = Expr.name("u32", s0)
        let a = Definition(label: "x", typeAnnotation: ann, value: .integerLiteral("1", s0), span: s0)
        let b = Definition(label: "x", typeAnnotation: nil, value: .integerLiteral("1", s0), span: s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - Expr

    func testExprIntegerLiteralEquality() {
        let a = Expr.integerLiteral("42", s0)
        let b = Expr.integerLiteral("42", s0)
        XCTAssertEqual(a, b)
    }

    func testExprFloatLiteralEquality() {
        let a = Expr.floatLiteral("3.14", s0)
        let b = Expr.floatLiteral("3.14", s0)
        XCTAssertEqual(a, b)
    }

    func testExprStringLiteralEquality() {
        let a = Expr.stringLiteral("hi", s0)
        let b = Expr.stringLiteral("hi", s0)
        XCTAssertEqual(a, b)
    }

    func testExprNameEquality() {
        let a = Expr.name("foo", s0)
        let b = Expr.name("foo", s0)
        XCTAssertEqual(a, b)
    }

    func testExprDiscardEquality() {
        let a = Expr.discard(s0)
        let b = Expr.discard(s0)
        XCTAssertEqual(a, b)
    }

    func testExprUnitEquality() {
        let a = Expr.unit(s0)
        let b = Expr.unit(s0)
        XCTAssertEqual(a, b)
    }

    func testExprDifferentKindsNotEqual() {
        let a = Expr.integerLiteral("0", s0)
        let b = Expr.floatLiteral("0", s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - Apply

    func testApplyEquality() {
        let a = Expr.apply(function: .name("add", s0), arguments: [.integerLiteral("1", s0), .integerLiteral("2", s1)], s0)
        let b = Expr.apply(function: .name("add", s0), arguments: [.integerLiteral("1", s0), .integerLiteral("2", s1)], s0)
        XCTAssertEqual(a, b)
    }

    func testApplyInequality() {
        let a = Expr.apply(function: .name("add", s0), arguments: [.integerLiteral("1", s0)], s0)
        let b = Expr.apply(function: .name("mul", s0), arguments: [.integerLiteral("1", s0)], s0)
        XCTAssertNotEqual(a, b)
    }

    func testApplyArgumentsInequality() {
        let a = Expr.apply(function: .name("f", s0), arguments: [.integerLiteral("1", s0)], s0)
        let b = Expr.apply(function: .name("f", s0), arguments: [.integerLiteral("2", s0)], s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - Pipe

    func testPipeEquality() {
        let a = Expr.pipe(clauses: [.integerLiteral("1", s0), .name("increment", s1)], s0)
        let b = Expr.pipe(clauses: [.integerLiteral("1", s0), .name("increment", s1)], s0)
        XCTAssertEqual(a, b)
    }

    func testPipeInequality() {
        let a = Expr.pipe(clauses: [.integerLiteral("1", s0), .name("increment", s1)], s0)
        let b = Expr.pipe(clauses: [.integerLiteral("2", s0), .name("increment", s1)], s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - Lambda

    func testLambdaEquality() {
        let a = Expr.lambda(param: "it", body: .name("it", s0), s0)
        let b = Expr.lambda(param: "it", body: .name("it", s0), s0)
        XCTAssertEqual(a, b)
    }

    func testLambdaInequality() {
        let a = Expr.lambda(param: "it", body: .name("it", s0), s0)
        let b = Expr.lambda(param: "x", body: .name("it", s0), s0)
        XCTAssertNotEqual(a, b)
    }

    func testLambdaBodyInequality() {
        let a = Expr.lambda(param: "it", body: .name("it", s0), s0)
        let b = Expr.lambda(param: "it", body: .name("other", s0), s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - CompoundKey

    func testCompoundKeyImplicit() {
        let a = CompoundKey.implicit
        let b = CompoundKey.implicit
        XCTAssertEqual(a, b)
    }

    func testCompoundKeyLabel() {
        let a = CompoundKey.label("a", s0)
        let b = CompoundKey.label("a", s0)
        XCTAssertEqual(a, b)
    }

    func testCompoundKeyLabelInequality() {
        let a = CompoundKey.label("a", s0)
        let b = CompoundKey.label("b", s0)
        XCTAssertNotEqual(a, b)
    }

    func testCompoundKeyIndex() {
        let a = CompoundKey.index("0", s0)
        let b = CompoundKey.index("0", s0)
        XCTAssertEqual(a, b)
    }

    func testCompoundKeyStringKey() {
        let a = CompoundKey.stringKey("key", s0)
        let b = CompoundKey.stringKey("key", s0)
        XCTAssertEqual(a, b)
    }

    // MARK: - CompoundEntry

    func testCompoundEntryEquality() {
        let a = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        let b = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        XCTAssertEqual(a, b)
    }

    func testCompoundEntryInequality() {
        let a = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        let b = CompoundEntry(key: .label("x", s0), value: .integerLiteral("1", s0), span: s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - Tuple

    func testTupleEquality() {
        let entry = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        let a = Expr.tuple(entries: [entry], s0)
        let b = Expr.tuple(entries: [entry], s0)
        XCTAssertEqual(a, b)
    }

    func testTupleInequality() {
        let e1 = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        let e2 = CompoundEntry(key: .implicit, value: .integerLiteral("2", s0), span: s0)
        let a = Expr.tuple(entries: [e1], s0)
        let b = Expr.tuple(entries: [e2], s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - Array

    func testArrayEquality() {
        let entry = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        let a = Expr.array(entries: [entry], s0)
        let b = Expr.array(entries: [entry], s0)
        XCTAssertEqual(a, b)
    }

    func testArrayInequality() {
        let e1 = CompoundEntry(key: .implicit, value: .integerLiteral("1", s0), span: s0)
        let e2 = CompoundEntry(key: .implicit, value: .integerLiteral("2", s0), span: s0)
        let a = Expr.array(entries: [e1], s0)
        let b = Expr.array(entries: [e2], s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - DotAccess

    func testDotAccessEquality() {
        let a = Expr.dotAccess(expr: .name("x", s0), field: "label", s0)
        let b = Expr.dotAccess(expr: .name("x", s0), field: "label", s0)
        XCTAssertEqual(a, b)
    }

    func testDotAccessInequality() {
        let a = Expr.dotAccess(expr: .name("x", s0), field: "label", s0)
        let b = Expr.dotAccess(expr: .name("x", s0), field: "other", s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - BracketAccess

    func testBracketAccessEquality() {
        let a = Expr.bracketAccess(expr: .name("x", s0), index: .integerLiteral("0", s0), s0)
        let b = Expr.bracketAccess(expr: .name("x", s0), index: .integerLiteral("0", s0), s0)
        XCTAssertEqual(a, b)
    }

    func testBracketAccessInequality() {
        let a = Expr.bracketAccess(expr: .name("x", s0), index: .integerLiteral("0", s0), s0)
        let b = Expr.bracketAccess(expr: .name("x", s0), index: .integerLiteral("1", s0), s0)
        XCTAssertNotEqual(a, b)
    }

    // MARK: - ComputedAccess

    func testComputedAccessEquality() {
        let a = Expr.computedAccess(expr: .name("x", s0), index: .integerLiteral("0", s0), s0)
        let b = Expr.computedAccess(expr: .name("x", s0), index: .integerLiteral("0", s0), s0)
        XCTAssertEqual(a, b)
    }

    func testComputedAccessInequality() {
        let a = Expr.computedAccess(expr: .name("x", s0), index: .integerLiteral("0", s0), s0)
        let b = Expr.computedAccess(expr: .name("y", s0), index: .integerLiteral("0", s0), s0)
        XCTAssertNotEqual(a, b)
    }
}
