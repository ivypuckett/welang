# Phase 13: Advanced Code Generation, Project Compilation, and Completion

## Goal

Complete the welang compiler by implementing code generation for all remaining constructs and building the end-to-end project compilation pipeline. This phase covers:

1. **Compound type codegen**: tuples/objects and arrays/maps as LLVM structs and arrays
2. **Pattern matching codegen**: conditional maps as branching control flow
3. **String operations codegen**: interpolated string concatenation, standard string operations
4. **Module linking**: combining multiple files into a single binary
5. **Object file emission and linking**: producing architecture-specific binaries
6. **External dependency fetching**: Go-style git-based dependency management
7. **CLI completion**: the `welang` command-line tool for building and running projects

After this phase, welang is a fully functional compiler:

```sh
# Compile and run a single file
welang run main.we

# Build a project to a binary
welang build

# Build and run
welang build && ./main
```

## Background

### Remaining Codegen

Phase 12 covered scalars, functions, application, and pipes. The following still need code generation:

- **Tuples/objects**: LLVM struct types with named fields
- **Arrays/maps**: heap-allocated, dynamically-sized collections
- **Dot access**: GEP (GetElementPtr) instructions for struct field access
- **Bracket access**: array indexing with bounds checking
- **Conditional maps**: branching (`br`, `phi` nodes) for pattern matching
- **Interpolated strings**: concatenation of string segments
- **Sum types**: tagged unions (tag byte + payload)
- **Macros**: already expanded before codegen (no IR needed)
- **Type annotations**: erased before codegen (no IR needed)

### Memory Model

welang is a functional language — values are immutable. This simplifies the memory model:

- **Stack allocation** for small values (scalars, small tuples)
- **Heap allocation** for dynamically-sized values (arrays, strings)
- **Reference counting** or **arena allocation** for heap memory (choose one)

For simplicity, this phase uses **arena allocation**: all heap memory is allocated from a single arena that is freed when the program exits. This avoids GC complexity while being correct for batch-style programs. A future enhancement could add reference counting.

### Linking Strategy

To produce a final binary:

1. Generate LLVM IR for each module
2. Link all modules into a single LLVM module (via `LLVMLinkModules2`)
3. Emit an object file from the linked module
4. Link with the system C runtime using `cc` (or `clang`)

## Project Context

### Files to Create/Modify

```
Sources/WeLangLib/
    Codegen.swift        ← extend with compound types, pattern matching, etc.
    Runtime.swift        ← NEW: runtime support functions (memory, strings)
    ProjectBuild.swift   ← NEW: project build pipeline
    Dependency.swift     ← NEW: external dependency management
    Compile.swift        ← update with full project compilation
Sources/WeLang/
    main.swift           ← update CLI with build/run subcommands
Sources/CLLLVM/
    include/shim.h       ← add any needed LLVM-C headers
Tests/WeLangTests/
    CodegenTests.swift   ← compound and pattern matching codegen tests
    CompileTests.swift   ← end-to-end compilation and execution tests
    ProjectBuildTests.swift ← NEW: project build pipeline tests
```

## Compound Type Codegen

### Tuples/Objects as LLVM Structs

```swift
func generateTuple(entries: [CompoundEntry], span: Span) throws -> LLVMValueRef {
    // Create struct type from entry types
    var fieldTypes: [LLVMTypeRef?] = []
    var fieldValues: [LLVMValueRef?] = []

    for entry in entries {
        let value = try generateExpr(entry.value)
        fieldTypes.append(LLVMTypeOf(value))
        fieldValues.append(value)
    }

    let structType = LLVMStructTypeInContext(context, &fieldTypes, UInt32(fieldTypes.count), 0)

    // Build struct value
    var structVal = LLVMGetUndef(structType)
    for (i, value) in fieldValues.enumerated() {
        structVal = LLVMBuildInsertValue(builder, structVal, value, UInt32(i), "field_\(i)")
    }

    return structVal!
}
```

### Dot Access (Struct Field Extraction)

```swift
func generateDotAccess(expr: Expr, field: String, span: Span) throws -> LLVMValueRef {
    let structVal = try generateExpr(expr)

    // Look up field index from the type info
    let fieldIndex = try resolveFieldIndex(field, in: expr)

    return LLVMBuildExtractValue(builder, structVal, UInt32(fieldIndex), "dot_\(field)")
}
```

### Arrays as Heap-Allocated Buffers

```swift
func generateArray(entries: [CompoundEntry], span: Span) throws -> LLVMValueRef {
    guard let firstEntry = entries.first else {
        // Empty array
        return generateEmptyArray()
    }

    // All elements have the same type (enforced by type checker)
    let elements = try entries.map { try generateExpr($0.value) }
    let elementType = LLVMTypeOf(elements[0])
    let count = entries.count

    // Allocate: { i64 length, [N x elementType] data }
    let arrayType = LLVMArrayType(elementType, UInt32(count))

    // Stack-allocate for small arrays, heap for large
    let alloca = LLVMBuildAlloca(builder, arrayType, "array")

    // Store elements
    for (i, elem) in elements.enumerated() {
        let indices: [LLVMValueRef?] = [
            LLVMConstInt(LLVMInt64TypeInContext(context), 0, 0),
            LLVMConstInt(LLVMInt64TypeInContext(context), UInt64(i), 0)
        ]
        var idxs = indices
        let gep = LLVMBuildGEP2(builder, arrayType, alloca, &idxs, 2, "elem_\(i)_ptr")
        LLVMBuildStore(builder, elem, gep)
    }

    return alloca!
}
```

### Bracket Access (Array Indexing)

```swift
func generateBracketAccess(expr: Expr, index: Expr, span: Span) throws -> LLVMValueRef {
    let arrayVal = try generateExpr(expr)
    let indexVal = try generateExpr(index)

    // TODO: bounds checking (optional, configurable)

    let elementType = getArrayElementType(arrayVal)
    var indices: [LLVMValueRef?] = [
        LLVMConstInt(LLVMInt64TypeInContext(context), 0, 0),
        indexVal
    ]
    let gep = LLVMBuildGEP2(builder, LLVMGetElementType(LLVMTypeOf(arrayVal)), arrayVal, &indices, 2, "index")
    return LLVMBuildLoad2(builder, elementType, gep, "elem")
}
```

## Pattern Matching Codegen

### Conditional Maps as Branching

Conditional maps compile to a chain of `if-then-else` branches:

```swift
func generateConditionalMap(branches: [ConditionalBranch], span: Span) throws -> LLVMValueRef {
    let function = LLVMGetBasicBlockParent(LLVMGetInsertBlock(builder))
    let mergeBlock = LLVMAppendBasicBlockInContext(context, function, "match_merge")

    var resultType: LLVMTypeRef? = nil
    var incomingValues: [LLVMValueRef?] = []
    var incomingBlocks: [LLVMBasicBlockRef?] = []

    for (i, branch) in branches.enumerated() {
        let isLast = i == branches.count - 1

        // Check for wildcard
        if isWildcard(branch.pattern) || isLast {
            // Default branch — unconditionally execute body
            let bodyVal = try generateExpr(branch.body)
            if resultType == nil { resultType = LLVMTypeOf(bodyVal) }
            incomingValues.append(bodyVal)
            incomingBlocks.append(LLVMGetInsertBlock(builder))
            LLVMBuildBr(builder, mergeBlock)
            break
        }

        // Evaluate predicate
        let predVal = try generateExpr(branch.pattern)

        let thenBlock = LLVMAppendBasicBlockInContext(context, function, "match_then_\(i)")
        let elseBlock = LLVMAppendBasicBlockInContext(context, function, "match_else_\(i)")

        LLVMBuildCondBr(builder, predVal, thenBlock, elseBlock)

        // Then block: evaluate body
        LLVMPositionBuilderAtEnd(builder, thenBlock)
        let bodyVal = try generateExpr(branch.body)
        if resultType == nil { resultType = LLVMTypeOf(bodyVal) }
        incomingValues.append(bodyVal)
        incomingBlocks.append(LLVMGetInsertBlock(builder))
        LLVMBuildBr(builder, mergeBlock)

        // Continue with else block
        LLVMPositionBuilderAtEnd(builder, elseBlock)
    }

    // Merge block with phi node
    LLVMPositionBuilderAtEnd(builder, mergeBlock)
    let phi = LLVMBuildPhi(builder, resultType, "match_result")
    LLVMAddIncoming(phi, &incomingValues, &incomingBlocks, UInt32(incomingValues.count))

    return phi!
}
```

### Sum Type Tagged Union Representation

```swift
// A sum type value is: { i8 tag, [maxPayloadSize x i8] payload }
func sumTypeLayout(variants: [(String, Type)]) -> (LLVMTypeRef, [String: (UInt8, LLVMTypeRef)]) {
    var maxSize: Int = 0
    var variantInfo: [String: (UInt8, LLVMTypeRef)] = [:]

    for (i, (name, type)) in variants.enumerated() {
        let llType = llvmType(for: type)
        let size = LLVMStoreSizeOfType(targetData, llType)
        maxSize = max(maxSize, Int(size))
        variantInfo[name] = (UInt8(i), llType)
    }

    // { i8, [maxSize x i8] }
    var fields: [LLVMTypeRef?] = [
        LLVMInt8TypeInContext(context),
        LLVMArrayType(LLVMInt8TypeInContext(context), UInt32(maxSize))
    ]
    let unionType = LLVMStructTypeInContext(context, &fields, 2, 0)

    return (unionType, variantInfo)
}
```

### Variant Construction

```swift
func generateVariantConstruction(typeName: String, variant: String, payload: Expr) throws -> LLVMValueRef {
    let tag = getVariantTag(typeName, variant)
    let payloadVal = try generateExpr(payload)

    // Allocate sum type
    let (unionType, _) = sumTypeLayout(for: typeName)
    let alloca = LLVMBuildAlloca(builder, unionType, "variant")

    // Store tag
    let tagPtr = LLVMBuildStructGEP2(builder, unionType, alloca, 0, "tag_ptr")
    LLVMBuildStore(builder, LLVMConstInt(LLVMInt8TypeInContext(context), UInt64(tag), 0), tagPtr)

    // Store payload (bitcast to byte array pointer, memcpy)
    let payloadPtr = LLVMBuildStructGEP2(builder, unionType, alloca, 1, "payload_ptr")
    let castPtr = LLVMBuildBitCast(builder, payloadPtr, LLVMPointerType(LLVMTypeOf(payloadVal), 0), "cast")
    LLVMBuildStore(builder, payloadVal, castPtr)

    return LLVMBuildLoad2(builder, unionType, alloca, "sum_val")
}
```

## String Operations

### Interpolated String Codegen

Interpolated strings were desugared into `.interpolatedString(segments:)` in Phase 5. Generate code by concatenating segments:

```swift
func generateInterpolatedString(segments: [InterpolationSegment]) throws -> LLVMValueRef {
    if segments.isEmpty {
        return LLVMBuildGlobalStringPtr(builder, "", "empty_str")
    }

    // Convert each segment to a string value
    var stringParts: [LLVMValueRef] = []
    for segment in segments {
        switch segment {
        case .text(let text, _):
            stringParts.append(LLVMBuildGlobalStringPtr(builder, text, "str_part")!)
        case .expression(let expr, _):
            let value = try generateExpr(expr)
            // Call toString on the value
            let strValue = try generateToStringCall(value)
            stringParts.append(strValue)
        }
    }

    // Concatenate all parts using runtime concat function
    return try generateStringConcat(stringParts)
}
```

### Runtime String Functions

Declare runtime support functions (implemented in C or as LLVM IR):

```swift
func declareRuntimeFunctions() {
    // welang_concat(str1: *i8, str2: *i8) -> *i8
    // welang_strlen(str: *i8) -> i64
    // welang_int_to_string(n: i64) -> *i8
    // welang_float_to_string(n: double) -> *i8
    // welang_alloc(size: i64) -> *i8
    // welang_print(str: *i8) -> void
}
```

These runtime functions can be:
1. Implemented as LLVM IR generated inline (self-contained)
2. Implemented in a C file that gets linked alongside the welang output
3. Calls to `libc` functions (`malloc`, `snprintf`, `puts`, `strlen`, etc.)

Option 3 (libc calls) is simplest:

```swift
func declareLibcFunctions() {
    // printf
    var printfArgs: [LLVMTypeRef?] = [LLVMPointerType(LLVMInt8TypeInContext(context), 0)]
    let printfType = LLVMFunctionType(LLVMInt32TypeInContext(context), &printfArgs, 1, 1) // variadic
    LLVMAddFunction(module, "printf", printfType)

    // malloc
    var mallocArgs: [LLVMTypeRef?] = [LLVMInt64TypeInContext(context)]
    let mallocType = LLVMFunctionType(LLVMPointerType(LLVMInt8TypeInContext(context), 0), &mallocArgs, 1, 0)
    LLVMAddFunction(module, "malloc", mallocType)

    // strlen, memcpy, snprintf, etc.
}
```

## Project Build Pipeline

### Build Steps

```swift
public func buildProject(root: String, output: String = "main") throws {
    // 1. Discover and parse all modules
    let project = try discoverModules(root: root)
    try validateProject(project)

    // 2. Fetch external dependencies (if any)
    try fetchDependencies(project: project, root: root)

    // 3. Compile in dependency order
    let order = try compilationOrder(project)
    var modules: [LLVMModuleRef] = []

    for module in order {
        let expanded = try expandMacros(module.program)
        let typed = try infer(expanded)
        let llvmModule = try generate(typed)
        modules.append(llvmModule)
    }

    // 4. Link all modules
    let linked = try linkModules(modules)

    // 5. Emit object file
    let objectPath = "\(root)/.build/\(output).o"
    try emitObjectFile(module: linked, to: objectPath)

    // 6. Link with system linker
    try linkBinary(objectFile: objectPath, output: "\(root)/\(output)")
}
```

### Module Linking

```swift
func linkModules(_ modules: [LLVMModuleRef]) throws -> LLVMModuleRef {
    guard let first = modules.first else {
        throw CompileError.noModules
    }

    for module in modules.dropFirst() {
        // LLVMLinkModules2 consumes the source module
        if LLVMLinkModules2(first, module) != 0 {
            throw CodegenError.llvmError(message: "Module linking failed")
        }
    }

    return first
}
```

### System Linking

```swift
func linkBinary(objectFile: String, output: String) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/cc")
    process.arguments = [objectFile, "-o", output, "-lm"]  // link math library

    try process.run()
    process.waitUntilExit()

    if process.terminationStatus != 0 {
        throw CompileError.linkerFailed(status: Int(process.terminationStatus))
    }
}
```

## External Dependency Management

### Fetching Dependencies

Go-style: clone git repos at specific tags.

```swift
public func fetchDependencies(project: Project, root: String) throws {
    let manifest = try parseManifest(at: "\(root)/we.toml")

    let depsDir = "\(root)/.deps"
    try FileManager.default.createDirectory(atPath: depsDir, withIntermediateDirectories: true)

    for dep in manifest {
        let depPath = "\(depsDir)/\(dep.name)"

        if FileManager.default.fileExists(atPath: depPath) {
            // Already fetched — verify tag
            try verifyTag(at: depPath, expected: dep.tag)
        } else {
            // Clone at specific tag
            try gitClone(url: dep.gitUrl, tag: dep.tag, to: depPath)
        }
    }
}

func gitClone(url: String, tag: String, to path: String) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["clone", "--branch", tag, "--depth", "1", url, path]

    try process.run()
    process.waitUntilExit()

    if process.terminationStatus != 0 {
        throw ModuleError.dependencyFetchFailed(url: url, tag: tag)
    }
}
```

## CLI Update

### Updated `main.swift`

```swift
import WeLangLib
import Foundation

let args = CommandLine.arguments

guard args.count >= 2 else {
    printUsage()
    exit(1)
}

let command = args[1]

switch command {
case "run":
    // welang run <file.we>
    guard args.count >= 3 else {
        print("Usage: welang run <file.we>")
        exit(1)
    }
    let file = args[2]
    let source = try String(contentsOfFile: file, encoding: .utf8)
    try compileAndRun(source)

case "build":
    // welang build [--output name]
    let root = FileManager.default.currentDirectoryPath
    let output = args.count >= 4 && args[2] == "--output" ? args[3] : "main"
    try buildProject(root: root, output: output)
    print("Built: ./\(output)")

case "check":
    // welang check — type-check without generating code
    let root = FileManager.default.currentDirectoryPath
    try checkProject(root: root)
    print("All checks passed.")

default:
    // Legacy: welang <file.we> — compile and run
    let file = command
    let source = try String(contentsOfFile: file, encoding: .utf8)
    try compileAndRun(source)
}

func printUsage() {
    print("""
    Usage: welang <command> [options]

    Commands:
      run <file.we>    Compile and run a single file
      build            Build the project in the current directory
      check            Type-check the project without building

    Options:
      --output <name>  Set the output binary name (default: main)
    """)
}
```

## Runtime Library

Create a minimal C runtime that gets linked with every welang binary:

```c
// runtime.c — welang runtime support
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

char* welang_concat(const char* a, const char* b) {
    size_t la = strlen(a), lb = strlen(b);
    char* result = malloc(la + lb + 1);
    memcpy(result, a, la);
    memcpy(result + la, b, lb + 1);
    return result;
}

char* welang_int_to_string(long long n) {
    char* buf = malloc(21);
    snprintf(buf, 21, "%lld", n);
    return buf;
}

char* welang_float_to_string(double n) {
    char* buf = malloc(32);
    snprintf(buf, 32, "%g", n);
    return buf;
}

void welang_print(const char* s) {
    puts(s);
}
```

This can be compiled once and bundled with the welang distribution, or generated as LLVM IR inline.

## Tests to Write

### Codegen Tests

**Tuple codegen:**
- `testCodegenTupleLiteral`: `{1, 2}` → generates struct with two fields
- `testCodegenTupleDotAccess`: `x.label` → generates extractvalue
- `testCodegenNestedTuple`: `{a: {b: 1}}` → nested struct

**Array codegen:**
- `testCodegenArrayLiteral`: `[1, 2, 3]` → generates array allocation and stores
- `testCodegenBracketAccess`: `x[0]` → generates GEP and load
- `testCodegenEmptyArray`: `[]` → valid empty array

**Pattern matching codegen:**
- `testCodegenConditionalMapTwoBranches`: `[(pred): a, (_): b]` → generates branch and phi
- `testCodegenConditionalMapMultiple`: three branches → chain of conditional branches
- `testCodegenWildcardOnly`: `[(_): 0]` → unconditional

**String codegen:**
- `testCodegenInterpolatedString`: `` `hello {{name}}` `` → generates concat calls
- `testCodegenStringConcat`: multiple string parts concatenated

**Sum type codegen:**
- `testCodegenSumTypeConstruction`: create a variant → tag + payload stored
- `testCodegenSumTypeMatch`: match on variant tag → correct branching

### Execution Tests (JIT)

- `testExecuteTupleAccess`: create tuple, access field → correct value
- `testExecuteArrayAccess`: create array, access element → correct value
- `testExecuteConditionalMap`: pattern match returns correct branch
- `testExecuteStringInterpolation`: interpolated string produces correct output
- `testExecuteMultipleDefinitions`: program with several definitions → correct result

### Project Build Tests

- `testBuildSingleFileProject`: single main.we builds to binary
- `testBuildMultiFileProject`: main.we imports helper.we → links correctly
- `testBuildLibProject`: lib.we without main → produces library object
- `testBuildWithDependencyManifest`: project with we.toml → dependencies handled

### CLI Tests

- `testCLIRun`: `welang run test.we` executes correctly
- `testCLIBuild`: `welang build` produces a binary
- `testCLICheck`: `welang check` reports type errors

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Tuples, arrays, and their access patterns generate correct LLVM IR.
4. Conditional maps generate correct branching control flow.
5. String interpolation generates concatenation calls.
6. Sum types generate tagged union representations.
7. Multi-file projects compile and link into a single binary.
8. The CLI supports `run`, `build`, and `check` subcommands.
9. Simple welang programs can be compiled to native binaries and executed.

## Important Notes

- **This is the largest phase**: it covers a lot of ground. Prioritize getting basic end-to-end execution working first (scalar types + functions + simple application), then add compound types and pattern matching incrementally.
- **Test incrementally**: after adding each major feature (tuples, arrays, conditionals), run the test suite before moving to the next feature.
- **The runtime library is minimal**: just string operations and memory allocation. Keep it simple — welang programs can call C functions for anything else.
- **Object file linking requires a C compiler** (`cc` or `clang`) on the system. This is standard for LLVM-based compilers.
- **External dependencies are a stretch goal**: if time is limited, defer the git-based dependency management. The module system itself (Phase 11) is the foundation.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
- **This completes the welang compiler**. After this phase, the language is functional and can compile programs to native binaries.
