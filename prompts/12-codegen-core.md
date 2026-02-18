# Phase 12: LLVM Code Generation — Core Types and Functions

## Goal

Implement LLVM IR generation for welang's core constructs, turning the type-checked AST into executable code. This phase covers:

1. **Scalar types**: integers (signed/unsigned), floats, strings, booleans, unit
2. **Function definitions**: monadic functions with the implicit `x` parameter
3. **Function application**: S-expression evaluation (curried calls)
4. **Pipe evaluation**: Forth-style left-to-right function composition
5. **Top-level definitions**: global constants and functions
6. **The `main` entry point**: program execution starts here

After this phase, welang programs can be compiled and executed:

```we
# This program prints the result of 2 + 3
main: (print (add 2 3))
```

## Background

### LLVM IR Basics

LLVM IR is a typed, SSA-form (Static Single Assignment) intermediate representation. Key concepts:

- **Modules**: top-level containers for functions and globals
- **Functions**: have a signature, basic blocks, and instructions
- **Basic blocks**: sequences of instructions ending with a terminator (ret, br, etc.)
- **Instructions**: operations that produce values (add, call, load, store, etc.)
- **Types**: i8, i16, i32, i64, float, double, pointer, struct, array, function

The welang compiler uses the **LLVM-C API** (via the `CLLLVM` Swift module) to build IR programmatically.

### LLVM-C API Pattern

```swift
import CLLLVM

// Create a function
let funcType = LLVMFunctionType(returnType, paramTypes, paramCount, /*isVarArg*/ 0)
let function = LLVMAddFunction(module, "myFunc", funcType)

// Create a basic block
let entry = LLVMAppendBasicBlockInContext(context, function, "entry")

// Position the builder
let builder = LLVMCreateBuilderInContext(context)
LLVMPositionBuilderAtEnd(builder, entry)

// Emit instructions
let result = LLVMBuildAdd(builder, lhs, rhs, "result")
LLVMBuildRet(builder, result)

// Clean up
LLVMDisposeBuilder(builder)
```

### Compilation Strategy

welang's codegen follows a straightforward strategy:

1. **Top-level definitions** become either LLVM global constants or LLVM functions.
2. **Functions** (definitions whose value references `x`, or lambda expressions) become LLVM functions with one parameter.
3. **Scalar literals** become LLVM constants.
4. **Function application** becomes LLVM `call` instructions.
5. **Pipes** are desugared into nested calls at the IR level.
6. **Lambdas** (`(name: body)`) are compiled as closures — anonymous functions with a captured environment.

### Currying via Closures

Since welang functions are always monadic, multi-argument functions use currying:

```we
add: (addImpl x)  # takes one arg, returns a function taking the second
```

At the LLVM level, curried functions are implemented as **closures**: a function pointer paired with a captured environment. This is the standard functional language compilation technique (see Appel's "Compiling with Continuations" or the STG machine used by GHC).

For this phase, we'll use a simplified closure representation:

```
struct Closure {
    funcPtr: ptr   // pointer to the underlying LLVM function
    envPtr: ptr    // pointer to captured variables (or null)
}
```

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    Codegen.swift    ← rewrite with real IR generation
    Compile.swift    ← update pipeline to use codegen output
    Errors.swift     ← extend CodegenError
Sources/CLLLVM/
    include/shim.h   ← may need additional LLVM-C headers
Tests/WeLangTests/
    CodegenTests.swift ← comprehensive codegen tests
    CompileTests.swift ← end-to-end execution tests
```

### Current Codegen State

The existing `Codegen.swift` has LLVM scaffolding (context, module, JIT engine creation) but no AST walking. The `generate()` function sets up an LLVM module and does nothing with the program.

### LLVM-C Headers Available

```c
#include <llvm-c/Core.h>
#include <llvm-c/ExecutionEngine.h>
#include <llvm-c/Target.h>
#include <llvm-c/Analysis.h>
```

You may need to add:
```c
#include <llvm-c/BitWriter.h>       // for writing bitcode
#include <llvm-c/TargetMachine.h>   // for emitting object files
```

## Codegen Architecture

### CodeGenerator Struct

```swift
public struct CodeGenerator {
    let context: LLVMContextRef
    let module: LLVMModuleRef
    let builder: LLVMBuilderRef

    /// Maps definition names to their LLVM values.
    var namedValues: [String: LLVMValueRef] = [:]

    /// Maps definition names to their inferred Types (from the type inference phase).
    var typeInfo: [String: Type] = [:]

    public init(context: LLVMContextRef, module: LLVMModuleRef) {
        self.context = context
        self.module = module
        self.builder = LLVMCreateBuilderInContext(context)
    }
}
```

### Top-Level Generation

```swift
public func generate(_ program: Program) throws -> LLVMModuleRef {
    let context = LLVMContextCreate()!
    let module = LLVMModuleCreateWithNameInContext("welang", context)!
    var codegen = CodeGenerator(context: context, module: module)

    // Declare built-in functions (add, multiply, print, etc.)
    codegen.declareBuiltins()

    // Generate each definition
    for def in program.definitions {
        try codegen.generateDefinition(def)
    }

    // Verify the module
    var error: UnsafeMutablePointer<CChar>?
    if LLVMVerifyModule(module, LLVMReturnStatusAction, &error) != 0 {
        let msg = error.map { String(cString: $0) } ?? "unknown"
        error.map { LLVMDisposeMessage($0) }
        throw CodegenError.llvmError(message: "Module verification failed: \(msg)")
    }

    return module
}
```

### Type Mapping

Map welang types to LLVM types:

```swift
func llvmType(for type: Type) -> LLVMTypeRef {
    switch type {
    case .primitive(.u8), .primitive(.i8):
        return LLVMInt8TypeInContext(context)
    case .primitive(.u16), .primitive(.i16):
        return LLVMInt16TypeInContext(context)
    case .primitive(.u32), .primitive(.i32):
        return LLVMInt32TypeInContext(context)
    case .primitive(.u64), .primitive(.i64):
        return LLVMInt64TypeInContext(context)
    case .primitive(.f32):
        return LLVMFloatTypeInContext(context)
    case .primitive(.f64):
        return LLVMDoubleTypeInContext(context)
    case .primitive(.bool):
        return LLVMInt1TypeInContext(context)
    case .primitive(.string):
        // Strings are pointers to i8
        return LLVMPointerType(LLVMInt8TypeInContext(context), 0)
    case .unit:
        return LLVMVoidTypeInContext(context)
    case .function(let input, let output):
        // Function types are closures (pointer to closure struct)
        return closureType()
    case .tuple(let fields):
        // Struct type
        var fieldTypes = fields.map { llvmType(for: $0.1) }
        return LLVMStructTypeInContext(context, &fieldTypes, UInt32(fieldTypes.count), 0)
    case .array(_, let value):
        // Array is { i64 length, ptr data }
        return arrayType(element: llvmType(for: value))
    case .variable(_):
        // Should be resolved by type inference — fallback to i64
        return LLVMInt64TypeInContext(context)
    case .nominal(_, let inner):
        // Same representation as inner type, with a tag
        return llvmType(for: inner)
    }
}
```

### Closure Type

```swift
func closureType() -> LLVMTypeRef {
    // { ptr funcPtr, ptr envPtr }
    var fields: [LLVMTypeRef?] = [
        LLVMPointerType(LLVMInt8TypeInContext(context), 0),  // function pointer
        LLVMPointerType(LLVMInt8TypeInContext(context), 0),  // environment pointer
    ]
    return LLVMStructTypeInContext(context, &fields, 2, 0)
}
```

## Generating Definitions

### Constant Definitions

Definitions whose values are literals and don't reference `x`:

```swift
func generateDefinition(_ def: Definition) throws {
    if referencesX(def.value) {
        try generateFunctionDefinition(def)
    } else {
        try generateConstantDefinition(def)
    }
}

func generateConstantDefinition(_ def: Definition) throws {
    let value = try generateExpr(def.value)
    namedValues[def.label] = value
}
```

### Function Definitions

```swift
func generateFunctionDefinition(_ def: Definition) throws {
    let inputType = llvmType(for: getInputType(def))
    let outputType = llvmType(for: getOutputType(def))

    // Create function type: inputType → outputType
    var paramTypes: [LLVMTypeRef?] = [inputType]
    let funcType = LLVMFunctionType(outputType, &paramTypes, 1, 0)
    let function = LLVMAddFunction(module, def.label, funcType)

    // Create entry block
    let entry = LLVMAppendBasicBlockInContext(context, function, "entry")
    LLVMPositionBuilderAtEnd(builder, entry)

    // Bind parameter to "x" (the implicit parameter name)
    let param = LLVMGetParam(function, 0)
    LLVMSetValueName2(param, "x", 1)
    namedValues["x"] = param

    // Generate body
    let result = try generateExpr(def.value)

    // Return result
    LLVMBuildRet(builder, result)

    // Store function reference
    namedValues[def.label] = function
}
```

### Lambda Expressions

Lambdas (`(name: body)`) are anonymous functions with a named parameter. They compile to closures — an LLVM function plus a captured environment:

```swift
func generateLambda(param: String, body: Expr) throws -> LLVMValueRef {
    // Save current state
    let savedValues = namedValues
    let savedBlock = LLVMGetInsertBlock(builder)

    // Determine types from type inference
    let inputType = llvmType(for: getLambdaInputType(param, body))
    let outputType = llvmType(for: getLambdaOutputType(param, body))

    // Create the lambda's LLVM function
    var paramTypes: [LLVMTypeRef?] = [inputType]
    let funcType = LLVMFunctionType(outputType, &paramTypes, 1, 0)
    let function = LLVMAddFunction(module, "lambda", funcType)

    let entry = LLVMAppendBasicBlockInContext(context, function, "entry")
    LLVMPositionBuilderAtEnd(builder, entry)

    // Bind the named parameter (e.g., "it" instead of "x")
    let paramVal = LLVMGetParam(function, 0)
    LLVMSetValueName2(paramVal, param, UInt32(param.utf8.count))
    namedValues[param] = paramVal

    // Generate body (outer scope names are still accessible for captures)
    let result = try generateExpr(body)
    LLVMBuildRet(builder, result)

    // Restore state
    namedValues = savedValues
    LLVMPositionBuilderAtEnd(builder, savedBlock)

    // Wrap as closure and return
    return try wrapAsClosure(function: function)
}
```

## Generating Expressions

```swift
func generateExpr(_ expr: Expr) throws -> LLVMValueRef {
    switch expr {
    case .integerLiteral(let text, _):
        let value = Int64(text) ?? 0
        return LLVMConstInt(LLVMInt64TypeInContext(context), UInt64(bitPattern: value), 1)

    case .floatLiteral(let text, _):
        let value = Double(text) ?? 0.0
        return LLVMConstReal(LLVMDoubleTypeInContext(context), value)

    case .stringLiteral(let text, _):
        return LLVMBuildGlobalStringPtr(builder, text, "str")

    case .name(let name, let span):
        guard let value = namedValues[name] else {
            throw CodegenError.undefinedReference(name: name)
        }
        return value

    case .unit(_):
        // Unit is represented as void or empty struct
        return LLVMGetUndef(LLVMStructTypeInContext(context, nil, 0, 0))

    case .apply(let function, let arguments, _):
        return try generateApply(function: function, arguments: arguments)

    case .pipe(let clauses, _):
        return try generatePipe(clauses: clauses)

    case .lambda(let param, let body, _):
        return try generateLambda(param: param, body: body)

    case .discard(_):
        return LLVMGetUndef(LLVMInt64TypeInContext(context))

    // tuple, array, conditionalMap, etc. — handled in Phase 13
    default:
        throw CodegenError.unsupportedExpr
    }
}
```

### Function Application

```swift
func generateApply(function: Expr, arguments: [Expr]) throws -> LLVMValueRef {
    var funcValue = try generateExpr(function)

    // Curried application: apply one argument at a time
    for arg in arguments {
        let argValue = try generateExpr(arg)

        // If funcValue is a direct function, call it
        // If funcValue is a closure, extract and call
        if LLVMIsAFunction(funcValue) != nil {
            var args: [LLVMValueRef?] = [argValue]
            funcValue = LLVMBuildCall2(
                builder,
                LLVMGetElementType(LLVMTypeOf(funcValue)),
                funcValue,
                &args,
                1,
                "call_result"
            )
        } else {
            // Closure call — extract function pointer and call with env + arg
            funcValue = try generateClosureCall(closure: funcValue, arg: argValue)
        }
    }

    return funcValue
}
```

### Pipe Evaluation

```swift
func generatePipe(clauses: [Expr]) throws -> LLVMValueRef {
    // Evaluate first clause
    var current = try generateExpr(clauses[0])

    // Thread through remaining clauses
    for clause in clauses.dropFirst() {
        let funcValue = try generateExpr(clause)

        // Apply current as the argument to funcValue
        if LLVMIsAFunction(funcValue) != nil {
            var args: [LLVMValueRef?] = [current]
            current = LLVMBuildCall2(
                builder,
                LLVMGetElementType(LLVMTypeOf(funcValue)),
                funcValue,
                &args,
                1,
                "pipe_result"
            )
        } else {
            current = try generateClosureCall(closure: funcValue, arg: current)
        }
    }

    return current
}
```

## Built-in Functions

Declare LLVM functions for built-in operations:

```swift
func declareBuiltins() {
    // add: i64 → i64 → i64 (curried — two nested functions)
    declareArithmeticBuiltin("add", op: { b, l, r in LLVMBuildAdd(b, l, r, "add") })
    declareArithmeticBuiltin("multiply", op: { b, l, r in LLVMBuildMul(b, l, r, "mul") })
    declareArithmeticBuiltin("subtract", op: { b, l, r in LLVMBuildSub(b, l, r, "sub") })

    // print: string → unit
    declarePrintBuiltin()

    // greaterThan: i64 → i64 → bool
    declareComparisonBuiltin("greaterThan", pred: LLVMIntSGT)

    // negate: i64 → i64
    declareUnaryBuiltin("negate", op: { b, v in LLVMBuildNeg(b, v, "neg") })

    // toString: a → string (polymorphic — handle for common types)
    declareToStringBuiltin()
}
```

For curried built-ins like `add`, you need a two-stage function:

```
add_outer(x) → returns closure { funcPtr: add_inner, env: {x} }
add_inner(env, y) → returns env.x + y
```

This is the standard technique for compiling curried functions.

### The `main` Function

If the program has a `main` definition, generate a C-compatible `main`:

```swift
func generateMainWrapper() throws {
    // Create: int main(int argc, char** argv)
    var paramTypes: [LLVMTypeRef?] = [
        LLVMInt32TypeInContext(context),                      // argc
        LLVMPointerType(LLVMPointerType(LLVMInt8TypeInContext(context), 0), 0)  // argv
    ]
    let mainType = LLVMFunctionType(LLVMInt32TypeInContext(context), &paramTypes, 2, 0)
    let mainFunc = LLVMAddFunction(module, "main", mainType)

    let entry = LLVMAppendBasicBlockInContext(context, mainFunc, "entry")
    LLVMPositionBuilderAtEnd(builder, entry)

    // Call the welang main definition
    if let welangMain = namedValues["main"] {
        // Pack argc/argv into args object and call welang main
        let argc = LLVMGetParam(mainFunc, 0)
        // ... pass args, call welangMain, return result
    }

    LLVMBuildRet(builder, LLVMConstInt(LLVMInt32TypeInContext(context), 0, 0))
}
```

## Output Modes

### JIT Execution (for testing)

Execute the program in-memory via LLVM's JIT:

```swift
public func executeJIT(module: LLVMModuleRef) throws -> Int32 {
    LLVMLinkInMCJIT()
    LLVM_InitializeNativeTarget()
    LLVM_InitializeNativeAsmPrinter()

    var engine: LLVMExecutionEngineRef?
    var error: UnsafeMutablePointer<CChar>?

    if LLVMCreateJITCompilerForModule(&engine, module, 0, &error) != 0 {
        // handle error
    }

    guard let mainFunc = LLVMGetNamedFunction(module, "main") else {
        throw CodegenError.noMainFunction
    }

    let result = LLVMRunFunction(engine, mainFunc, 0, nil)
    return Int32(LLVMGenericValueToInt(result, 1))
}
```

### Object File Emission (for linking)

```swift
public func emitObjectFile(module: LLVMModuleRef, to path: String) throws {
    // Initialize target
    LLVMInitializeAllTargetInfos()
    LLVMInitializeAllTargets()
    LLVMInitializeAllTargetMCs()
    LLVMInitializeAllAsmPrinters()

    let triple = LLVMGetDefaultTargetTriple()
    defer { LLVMDisposeMessage(triple) }

    var target: LLVMTargetRef?
    var error: UnsafeMutablePointer<CChar>?
    LLVMGetTargetFromTriple(triple, &target, &error)

    let machine = LLVMCreateTargetMachine(
        target, triple, "generic", "",
        LLVMCodeGenLevelDefault, LLVMRelocDefault, LLVMCodeModelDefault
    )

    LLVMTargetMachineEmitToFile(machine, module, strdup(path), LLVMObjectFile, &error)
}
```

## Error Extensions

```swift
public enum CodegenError: Error, Equatable, CustomStringConvertible {
    case llvmError(message: String)
    case undefinedReference(name: String)
    case unsupportedExpr
    case noMainFunction
    case typeMappingFailed(Type)
}
```

## Tests to Write

### Codegen Tests

**Scalar codegen:**
- `testCodegenIntegerLiteral`: emit an integer constant → verify IR contains the constant
- `testCodegenFloatLiteral`: emit a float constant
- `testCodegenStringLiteral`: emit a global string pointer
- `testCodegenBoolLiteral`: emit i1 constant

**Function codegen:**
- `testCodegenSimpleFunction`: `"double: (multiply x 2)"` → emits a function with one parameter
- `testCodegenFunctionWithReturn`: function returns computed value
- `testCodegenIdentityFunction`: `"id: x"` → function that returns its parameter

**Lambda codegen:**
- `testCodegenLambda`: `"f: (it: it)"` → emits an anonymous function with one parameter named "it"
- `testCodegenLambdaWithBody`: `"f: (it: multiply it 2)"` → lambda function with call in body
- `testCodegenLambdaAsArgument`: `"r: (map (it: multiply it 2) list)"` → lambda passed as closure argument
- `testCodegenLambdaCapture`: `"f: (it: add it x)"` → lambda captures outer `x`

**Application codegen:**
- `testCodegenApply`: `"r: (add 1 2)"` → emits call instruction
- `testCodegenNestedApply`: `"r: (add (multiply 2 3) 4)"` → nested calls

**Pipe codegen:**
- `testCodegenPipe`: `"r: (1 | double)"` → emits chained call

**Module verification:**
- `testCodegenModuleVerifies`: generated module passes LLVM verification
- `testCodegenEmptyProgram`: empty program → valid module (existing test, updated)

### Execution Tests (JIT)

- `testExecuteIntegerReturn`: program that returns an integer via main → correct return value
- `testExecuteArithmetic`: `(add 2 3)` → returns 5
- `testExecuteNestedArithmetic`: `(add (multiply 2 3) 4)` → returns 10
- `testExecutePipe`: `(2 | double)` → returns 4

### Compile Tests

- `testCompileAndExecuteSimple`: full pipeline from source to JIT execution
- `testCompileAndVerify`: full pipeline produces a valid LLVM module

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Integer, float, string, and boolean literals generate correct LLVM constants.
4. Function definitions generate LLVM functions with correct signatures.
5. Function application generates call instructions.
6. Pipe expressions generate chained calls.
7. The generated LLVM module passes verification.
8. Simple programs execute correctly via JIT.
9. Lambda expressions compile to closures with correct parameter binding.

## Important Notes

- **Use the LLVM-C API exclusively**: Swift calls C functions from the `CLLLVM` module. All LLVM interaction goes through this API.
- **SSA form**: LLVM requires SSA. Use `LLVMBuildAlloca` + `LLVMBuildStore`/`LLVMBuildLoad` for mutable values, or better, use the SSA builder directly since welang is functional (no mutation).
- **Closure representation is critical**: getting currying right requires proper closure creation and calling conventions. Start with a simple approach (no closures — just direct calls for non-curried functions) and add closures incrementally.
- **Memory management**: be careful with LLVM-C memory. Use `defer` for `LLVMDispose*` calls. The module is owned by the caller after `generate()` returns.
- **Test IR structure, not text**: instead of comparing IR strings, test properties (e.g., "module has a function named X with return type Y"). This is more robust than string matching.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
