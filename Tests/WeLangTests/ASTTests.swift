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

    // MARK: - Application

    func testExprApplicationEquality() {
        let a = Expr.application(.name("f", s0), .name("x", s1), s0)
        let b = Expr.application(.name("f", s0), .name("x", s1), s0)
        XCTAssertEqual(a, b)
    }

    func testExprApplicationInequalityByFn() {
        let a = Expr.application(.name("f", s0), .name("x", s1), s0)
        let b = Expr.application(.name("g", s0), .name("x", s1), s0)
        XCTAssertNotEqual(a, b)
    }

    func testExprApplicationInequalityByArg() {
        let a = Expr.application(.name("f", s0), .name("x", s1), s0)
        let b = Expr.application(.name("f", s0), .name("y", s1), s0)
        XCTAssertNotEqual(a, b)
    }

    func testExprApplicationSpan() {
        let a = Expr.application(.name("f", s0), .name("x", s1), s1)
        XCTAssertEqual(a.span, s1)
    }

    func testExprApplicationIsIndirect() {
        // Nested application — verifies the `indirect` keyword allows recursion.
        let inner = Expr.application(.name("g", s0), .name("h", s0), s0)
        let outer = Expr.application(.name("f", s0), inner, s0)
        guard case .application(_, let arg, _) = outer,
              case .application(let g, let h, _) = arg else {
            XCTFail("Expected nested application"); return
        }
        guard case .name("g", _) = g, case .name("h", _) = h else {
            XCTFail("Unexpected names"); return
        }
    }

    // MARK: - Pipe

    func testExprPipeEquality() {
        let a = Expr.pipe(.name("x", s0), .name("f", s1), s0)
        let b = Expr.pipe(.name("x", s0), .name("f", s1), s0)
        XCTAssertEqual(a, b)
    }

    func testExprPipeInequalityByInput() {
        let a = Expr.pipe(.name("x", s0), .name("f", s1), s0)
        let b = Expr.pipe(.name("y", s0), .name("f", s1), s0)
        XCTAssertNotEqual(a, b)
    }

    func testExprPipeInequalityByFn() {
        let a = Expr.pipe(.name("x", s0), .name("f", s1), s0)
        let b = Expr.pipe(.name("x", s0), .name("g", s1), s0)
        XCTAssertNotEqual(a, b)
    }

    func testExprPipeSpan() {
        let a = Expr.pipe(.name("x", s0), .name("f", s1), s1)
        XCTAssertEqual(a.span, s1)
    }

    func testExprPipeIsIndirect() {
        // Chained pipes — verifies the `indirect` keyword allows recursion.
        let inner = Expr.pipe(.name("a", s0), .name("b", s0), s0)
        let outer = Expr.pipe(inner, .name("c", s0), s0)
        guard case .pipe(let lhs, let c, _) = outer,
              case .pipe(let a, let b, _) = lhs else {
            XCTFail("Expected nested pipe"); return
        }
        guard case .name("a", _) = a, case .name("b", _) = b, case .name("c", _) = c else {
            XCTFail("Unexpected names"); return
        }
    }

    func testExprApplicationNotEqualToPipe() {
        let a = Expr.application(.name("f", s0), .name("x", s1), s0)
        let b = Expr.pipe(.name("f", s0), .name("x", s1), s0)
        XCTAssertNotEqual(a, b)
    }
}
