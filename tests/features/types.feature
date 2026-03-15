Feature: Type system

  Scenario: primitive i64 structural type acts as identity at runtime
    Given the welang definitions:
      """
      anyInt: 'i64
      """
    Given the welang expression "(anyInt 42)"
    Then it should evaluate to 42

  Scenario: primitive f64 structural type acts as identity at runtime
    Given the welang definitions:
      """
      anyFloat: 'f64
      """
    Given the welang expression "(anyFloat 42)"
    Then it should evaluate to 42

  Scenario: bool structural type acts as identity at runtime
    Given the welang definitions:
      """
      anyBool: 'bool
      """
    Given the welang expression "(anyBool 1)"
    Then it should evaluate to 1

  Scenario: array structural type compiles successfully
    Given the welang definitions:
      """
      anyIntArray: '[i64]
      """
    Then it should compile successfully

  Scenario: map structural type compiles successfully
    Given the welang definitions:
      """
      anyMap: '{k1: bool, k2: i64}
      """
    Then it should compile successfully

  Scenario: function structural type compiles successfully
    Given the welang definitions:
      """
      anyFunction: '(i64 | i64)
      """
    Then it should compile successfully

  Scenario: discard structural type compiles successfully
    Given the welang definitions:
      """
      functionWithDiscard: '(_ | _)
      """
    Then it should compile successfully

  Scenario: inline type annotation does not change function behavior
    Given the welang definitions:
      """
      id '(i64 | i64): x
      """
    Then calling "id" with 5 should return 5

  Scenario: named type annotation does not change function behavior
    Given the welang definitions:
      """
      anyFloat: 'f64
      functionUsage: (anyFloat 99)
      """
    Given the welang expression "(functionUsage 0)"
    Then it should evaluate to 99

  Scenario: nominal i64 type acts as identity at runtime
    Given the welang definitions:
      """
      specialInt: *i64
      """
    Given the welang expression "(specialInt 5)"
    Then it should evaluate to 5

  Scenario: nominal constructor with zero returns zero
    Given the welang definitions:
      """
      specialInt: *i64
      """
    Given the welang expression "(specialInt 0)"
    Then it should evaluate to 0

  Scenario: function annotated with nominal type returns its body value
    Given the welang definitions:
      """
      specialInt: *i64
      z specialInt: 1
      """
    Then calling "z" with 0 should return 1

  Scenario: unconstrained generic structural type acts as identity
    Given the welang definitions:
      """
      genericId: '<T _> (T | T)
      """
    Given the welang expression "(genericId 42)"
    Then it should evaluate to 42

  Scenario: constrained generic structural type acts as identity for i64
    Given the welang definitions:
      """
      intId: '<T i64> (T | T)
      """
    Given the welang expression "(intId 42)"
    Then it should evaluate to 42

  Scenario: generic map structural type compiles successfully
    Given the welang definitions:
      """
      pairOfSame: '<T _> {k1: T, k2: T}
      """
    Then it should compile successfully

  Scenario: multiple generic parameters compile successfully
    Given the welang definitions:
      """
      multiGeneric: '<T i64, U _> {k1: T, k2: U}
      """
    Then it should compile successfully

  Scenario: nested generic constraint compiles successfully
    Given the welang definitions:
      """
      nestedGeneric: '<T i64, U <V _>{k1: V}> {k1: T, k2: U, k3: string}
      """
    Then it should compile successfully

  Scenario: generic specialization via body reference compiles and runs
    Given the welang program "generic-specialization.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: nominal generic type compiles and runs
    Given the welang program "nominal-types.we"
    When I compile and run it
    Then it should exit successfully
