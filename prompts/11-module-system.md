# Phase 11: Module System — Imports, Exports, and Visibility

## Goal

Implement welang's file-based module system, including:

1. **`pub` exports**: every file exports exactly one construct via a definition named `pub`
2. **Relative imports**: files are imported by path relative to the project root
3. **Visibility rules**: hierarchical visibility based on filesystem position
4. **External library imports**: Go-style dependency management via git tags
5. **File resolution and loading**: finding and reading imported modules

After this phase:

```we
# math.we — exports a math utilities object
pub: {
  add: (addImpl x),
  multiply: (multiplyImpl x),
  pi: 3.14159
}
```

```we
# main.we — imports math.we
result: (math.add 1 2)
# `math` refers to the pub export of math.we
```

## Background

### One File, One Export

welang's module system is radically simple: **every file exports exactly one thing** through a definition named `pub`. This is inspired by:

- **CommonJS**: `module.exports = ...` (one export per file)
- **Go**: package-level exports
- **ML**: module signatures (one signature per module)

The exported construct is typically a **tuple/object** containing the file's public API:

```we
# string_utils.we
pub: {
  toUpper: (toUpperImpl x),
  toLower: (toLowerImpl x),
  length: (lengthImpl x)
}
```

### Import by Filename

Imports are **implicit**: any file in the project can reference other files by their filename (without extension). The module's `pub` export becomes available under that name:

```we
# Uses string_utils.we's pub export
result: (string_utils.toUpper "hello")
```

There is no `import` keyword. The compiler resolves undefined names by looking for matching files.

### Named Exports

By default, a module is referenced by its filename. Named exports allow overriding:

```we
# This file can be imported as "su" instead of "string_utils"
pub: {
  _name: "su",
  toUpper: (toUpperImpl x),
  toLower: (toLowerImpl x)
}
```

The `_name` field (if present) in the `pub` tuple specifies the import name. This is the only special field.

### Visibility Rules

welang's visibility model is based on filesystem hierarchy. A file can see:

1. **Parent exports**: the `pub` of files in the same directory, and all ancestor directories up to the project root
2. **Sibling exports**: the `pub` of files in subdirectories of parent directories (the parent's children)
3. **Child exports**: the `pub` of files in immediate subdirectories of the current file's directory

A file **cannot** see:

4. **Grandchildren**: files in subdirectories of subdirectories (depth > 1)
5. **Nieces/nephews**: files in subdirectories of siblings

Visual example:
```
project/
├── main.we           ← can see: lib.we, utils/, math/
├── lib.we
├── utils/
│   ├── string.we     ← can see: main.we, lib.we, helpers/ (child)
│   │                    cannot see: math/ (sibling's children)
│   └── helpers/
│       └── trim.we   ← can see: string.we, main.we, lib.we
│                        cannot see: helpers/sub/ (grandchild)
└── math/
    ├── basic.we      ← can see: main.we, lib.we, advanced/ (child)
    │                    cannot see: utils/ (sibling)
    └── advanced/
        └── calc.we   ← can see: basic.we, main.we, lib.we
```

### External Libraries

External dependencies work like Go modules:

- No central package repository
- Dependencies are referenced by git URL and tag
- External modules are placed at the project root level, so all files can see them
- Managed via a manifest file (e.g., `we.toml` or `we.json`)

## Project Context

### Files to Create/Modify

```
Sources/WeLangLib/
    ModuleSystem.swift   ← NEW: module resolution, loading, visibility
    AST.swift            ← add import-related fields to Program
    Parser.swift         ← (minimal changes — no import keyword)
    TypeInference.swift  ← resolve cross-module name references
    Compile.swift        ← multi-file compilation pipeline
    Errors.swift         ← module-specific errors
Tests/WeLangTests/
    ModuleSystemTests.swift ← NEW: module resolution and visibility tests
    CompileTests.swift      ← multi-file compilation tests
```

## Module Resolution

### Project Structure Detection

```swift
/// Represents a welang project.
public struct Project {
    /// Root directory of the project.
    public let root: String

    /// All source files, keyed by their module path (relative to root, without .we extension).
    public var modules: [String: Module] = [:]

    public init(root: String) { ... }
}

/// A single welang source module (file).
public struct Module: Equatable {
    /// The module's path relative to project root (e.g., "utils/string").
    public let path: String

    /// The module's export name (filename by default, or overridden by _name).
    public let name: String

    /// The parsed AST.
    public let program: Program

    /// The file's directory relative to project root (e.g., "utils").
    public let directory: String

    public init(path: String, name: String, program: Program, directory: String) { ... }
}
```

### Module Discovery

```swift
/// Discover all .we files in the project and parse them.
public func discoverModules(root: String) throws -> Project {
    var project = Project(root: root)

    // Recursively find all .we files
    let files = findWeFiles(in: root)

    for file in files {
        let relativePath = file.removingPrefix(root + "/").removingSuffix(".we")
        let source = try String(contentsOfFile: file, encoding: .utf8)
        let tokens = try lex(source)
        let program = try parse(tokens)

        let name = extractModuleName(from: program) ?? filename(from: relativePath)
        let directory = directoryOf(relativePath)

        let module = Module(path: relativePath, name: name, program: program, directory: directory)
        project.modules[relativePath] = module
    }

    return project
}
```

### Extract Module Name

If the `pub` definition contains a `_name` field, use that as the module name:

```swift
func extractModuleName(from program: Program) -> String? {
    // Find the `pub` definition
    guard let pubDef = program.definitions.first(where: { $0.label == "pub" }) else {
        return nil
    }

    // If the value is a tuple with a _name field, extract it
    if case .tuple(let entries, _) = pubDef.value {
        for entry in entries {
            if case .label("_name", _) = entry.key,
               case .stringLiteral(let name, _) = entry.value {
                return name
            }
        }
    }

    return nil
}
```

### Visibility Checking

```swift
/// Determine which modules are visible from a given module.
public func visibleModules(from module: Module, in project: Project) -> [Module] {
    var visible: [Module] = []
    let currentDir = module.directory

    for (_, other) in project.modules {
        if other.path == module.path { continue }  // skip self
        if isVisible(from: currentDir, to: other, in: project) {
            visible.append(other)
        }
    }

    return visible
}

/// Check if `target` module is visible from `currentDir`.
func isVisible(from currentDir: String, to target: Module, in project: Project) -> Bool {
    let targetDir = target.directory

    // 1. Parent: target is in same directory or any ancestor
    if isAncestorOrSame(currentDir, of: currentDir) && isSameDir(targetDir, currentDir) {
        return true
    }

    // Walk up to root, checking each ancestor's direct children
    var dir = currentDir
    while true {
        // Files in this directory are visible
        if targetDir == dir {
            return true
        }

        // Immediate subdirectories of this directory are visible (siblings/children)
        if isImmediateSubdirectory(targetDir, of: dir) {
            return true
        }

        // Move up
        if dir.isEmpty || dir == "." {
            break
        }
        dir = parentDirectory(dir)
    }

    // External modules (at root level) are always visible
    if targetDir.isEmpty || targetDir == "." {
        return true
    }

    return false
}

/// Check if `child` is an immediate subdirectory of `parent`.
func isImmediateSubdirectory(_ child: String, of parent: String) -> Bool {
    // child should be parent + "/" + one segment (no further slashes)
    guard child.hasPrefix(parent.isEmpty ? "" : parent + "/") else { return false }
    let suffix = child.isEmpty ? child : String(child.dropFirst(parent.isEmpty ? 0 : parent.count + 1))
    return !suffix.contains("/")
}
```

### Visibility Rules Summary

From a file at path `a/b/file.we` (directory `a/b`):

| Target location | Visible? | Reason |
|----------------|----------|--------|
| `a/b/other.we` | Yes | Same directory (sibling) |
| `a/b/sub/mod.we` | Yes | Immediate subdirectory (child) |
| `a/b/sub/deep/mod.we` | **No** | Grandchild (depth > 1) |
| `a/other.we` | Yes | Parent directory |
| `a/c/mod.we` | Yes | Sibling directory of parent (uncle) |
| `a/c/d/mod.we` | **No** | Niece (subdirectory of sibling) |
| `root.we` | Yes | Ancestor (root) |
| `ext_lib/mod.we` | Yes | Root-level (external library) |

## Name Resolution

### Resolving Undefined Names

When the type checker encounters an undefined name, it should check if the name matches a visible module:

```swift
func resolveName(_ name: String, in module: Module, project: Project) throws -> TypeScheme? {
    // 1. Check local definitions
    if let local = localEnv.lookup(name) { return local }

    // 2. Check visible modules
    let visible = visibleModules(from: module, in: project)
    for mod in visible {
        if mod.name == name {
            // Return the type of the module's pub export
            return try getModuleExportType(mod)
        }
    }

    // 3. Not found
    return nil
}
```

### Module Export Types

The `pub` definition's value determines the module's export type. This is inferred by running type inference on the module first (topological sort of module dependencies).

### Compilation Order

Modules must be compiled in dependency order:

```swift
/// Topological sort of modules by their dependency graph.
public func compilationOrder(_ project: Project) throws -> [Module] {
    // Build dependency graph
    var graph: [String: Set<String>] = [:]  // module path → set of dependency paths

    for (path, module) in project.modules {
        let deps = findDependencies(module, in: project)
        graph[path] = Set(deps.map { $0.path })
    }

    // Topological sort (Kahn's algorithm)
    return try topologicalSort(graph, modules: project.modules)
}
```

### Circular Dependency Detection

Circular dependencies are an error:

```swift
case circularDependency(cycle: [String])
```

## Pipeline Update

### Multi-File Compilation

Update `Compile.swift` to support compiling a project (multiple files):

```swift
/// Compile a single file (existing behavior).
public func compile(_ source: String) throws {
    let tokens = try lex(source)
    let ast = try parse(tokens)
    let expanded = try expandMacros(ast)
    let typedAst = try infer(expanded)
    try generate(typedAst)
}

/// Compile a project (multi-file).
public func compileProject(root: String) throws {
    let project = try discoverModules(root: root)

    // Validate project structure
    try validateProject(project)

    // Compile in dependency order
    let order = try compilationOrder(project)
    var moduleTypes: [String: TypeEnv] = [:]

    for module in order {
        let expanded = try expandMacros(module.program)
        let env = try inferModule(expanded, visible: visibleModules(from: module, in: project), moduleTypes: moduleTypes)
        moduleTypes[module.path] = env
    }

    // Generate code for each module
    for module in order {
        try generate(module.program)
    }
}
```

### Project Validation

```swift
func validateProject(_ project: Project) throws {
    // Must have main.we with a main function OR lib.we with a pub definition
    let hasMain = project.modules.values.contains { $0.path == "main" || $0.path.hasSuffix("/main") }
    let hasLib = project.modules.values.contains { $0.path == "lib" || $0.path.hasSuffix("/lib") }

    if !hasMain && !hasLib {
        throw CompileError.noEntryPoint
    }

    // main.we must have a "main" definition that takes args and returns integer
    if hasMain {
        // Validated during type inference
    }

    // lib.we must have a "pub" definition
    if hasLib && !hasMain {
        // Validated during type inference
    }
}
```

## Error Cases

```swift
public enum ModuleError: Error, Equatable, CustomStringConvertible {
    case moduleNotFound(name: String, from: String)
    case circularDependency(cycle: [String])
    case visibilityViolation(target: String, from: String)
    case missingPubExport(module: String)
    case noEntryPoint
    case duplicateModuleName(name: String, paths: [String])
}
```

Update `CompileError` to include `.module(ModuleError)`.

## External Library Manifest

Define a simple manifest format (e.g., `we.toml`):

```toml
[dependencies]
json = { git = "https://github.com/user/we-json", tag = "v1.0.0" }
http = { git = "https://github.com/user/we-http", tag = "v2.1.0" }
```

For this phase, parsing the manifest is sufficient — actual git cloning is Phase 12.

```swift
public struct Dependency: Equatable {
    public let name: String
    public let gitUrl: String
    public let tag: String
}

public func parseManifest(at path: String) throws -> [Dependency] {
    // Simple TOML-like parsing for [dependencies] section
    ...
}
```

## Tests to Write

### Module System Tests (new file)

**Visibility:**
- `testVisibilitySameDirectory`: file sees siblings in same directory
- `testVisibilityParentDirectory`: file sees files in parent directory
- `testVisibilityChildDirectory`: file sees files in immediate subdirectory
- `testVisibilityGrandchildHidden`: file cannot see files two levels down
- `testVisibilityNieceHidden`: file cannot see subdirectories of siblings
- `testVisibilityRootAlwaysVisible`: root-level files are visible to all
- `testVisibilityExternalLibrary`: external libraries (root level) are visible to all

**Module resolution:**
- `testResolveModuleByFilename`: `math` resolves to `math.we`
- `testResolveModuleByNamedExport`: file with `_name: "su"` resolves as `su`
- `testResolveModuleNotFound`: undefined module name → error
- `testResolveModuleVisibilityViolation`: accessing hidden module → error

**Compilation order:**
- `testTopologicalSortNoDependencies`: independent files → any order
- `testTopologicalSortLinearDependency`: A → B → C → sorted as C, B, A
- `testTopologicalSortDiamondDependency`: diamond shape → valid topological order
- `testCircularDependencyDetection`: A → B → A → error

**Project validation:**
- `testValidateProjectWithMain`: project with main.we → valid
- `testValidateProjectWithLib`: project with lib.we → valid
- `testValidateProjectNoEntry`: project without main or lib → error

**Pub export:**
- `testExtractPubExport`: file with `pub: {add: ...}` → correctly extracted
- `testMissingPub`: file without `pub` definition → error
- `testNamedExport`: `pub` with `_name: "custom"` → module name is "custom"

### Compile Tests

- `testCompileMultiFileProject`: a two-file project compiles
- `testCompileCrossModuleReference`: file A references file B's export → works

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Module visibility rules enforce the hierarchy (parents, siblings, children — not grandchildren or nieces).
4. Modules are compiled in dependency order.
5. Circular dependencies are detected and reported.
6. The `pub` export mechanism works correctly.
7. Named exports override the default filename-based import name.
8. Project validation ensures main.we or lib.we exists.

## Important Notes

- **No `import` keyword**: modules are resolved by name lookup. This keeps the syntax minimal.
- **Single-file compilation still works**: the existing `compile(_ source: String)` function continues to work for single-file programs.
- **External library support is partial**: this phase parses the manifest but does not clone repositories. Actual git operations are Phase 12.
- **Module type inference is sequential**: modules are inferred in dependency order. Each module's type environment feeds into its dependents.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
