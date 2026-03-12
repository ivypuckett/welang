# Primitives

## Types

- `int` — integer (also used for booleans: 0 is false, non-zero is true)
- `str` — string literal
- `tuple(T)` — fixed pair `[a, b]` where both elements are type `T`
- `map(T)` — map literal `{k: v, …}` where all values are type `T`
- `α → β` — function from type `α` to type `β`

## Operations

- `add` — adds two integers: `(add [a, b])`
- `subtract` — subtracts two integers: `(subtract [a, b])`
- `multiply` — multiplies two integers: `(multiply [a, b])`
- `divide` — divides two integers: `(divide [a, b])`
- `equal` — returns 1 if two values are equal, 0 otherwise: `(equal [a, b])`
- `lessThan` — returns 1 if a < b, 0 otherwise: `(lessThan [a, b])`
- `greaterThan` — returns 1 if a > b, 0 otherwise: `(greaterThan [a, b])`
- `lessThanOrEqual` — returns 1 if a ≤ b, 0 otherwise: `(lessThanOrEqual [a, b])`
- `greaterThanOrEqual` — returns 1 if a ≥ b, 0 otherwise: `(greaterThanOrEqual [a, b])`
- `print` — prints an integer or string to stdout: `(print x)`
- `get` — retrieves a value from a map by key: `(get [map, key])`
