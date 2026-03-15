Feature: Generic type runtime behavior

  # Requirements from generics.we: constrained generics, generic map types,
  # multiple generic parameters, and nested generic constraints are defined
  # but not called in main.

  Scenario: Constrained generic acts as identity for i64
    Given the welang program "int-id.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Generic map type acts as identity
    Given the welang program "pair-of-same.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Multiple generic parameters
    Given the welang program "multi-generic-runtime.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Nested generic constraint
    Given the welang program "nested-generic-runtime.we"
    When I compile and run it
    Then it should exit successfully
