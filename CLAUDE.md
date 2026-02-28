# CLAUDE.md

## About welang

welang is a functional programming language implemented in Gleam. The lexer is
complete; the parser and interpreter are in progress.

## Key Language Concept: Always-Monadic Functions

**All functions in welang are monadic — exactly one input, one output.**

This fundamentally changes how expressions are read. In an S-expression like
`(add 1 2)`, the intuitive reading of "add 1 and 2" is **wrong**. The correct
reading is sequential application, right to left through the argument list:

1. Start with `2`
2. Pass `2` to `1` → produces a result
3. Pass that result to `add` → produces the final value

Equivalently with pipes: `2 | 1 | add`

The same applies to longer chains. `(print increment 1)` means: take `1`, pass
it to `increment`, pass that result to `print`.

When editing or extending the language, never assume multi-argument function
application. Every call site has exactly one data value flowing through it.

## Development

```sh
gleam run   # Run the project
gleam test  # Run the tests
gleam build # Build the project
```
