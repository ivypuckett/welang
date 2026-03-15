Feature: Structural type runtime semantics

  # Requirements from structural-types.we: most structural types are defined
  # but only (anyInt 0) is verified in main. Inline and named annotations
  # are also untested at runtime.

  Scenario: Float structural type is identity
    Given the welang program "structural-float.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Bool structural type is identity
    Given the welang program "structural-bool.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Array structural type is identity
    Given the welang program "structural-array.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Function structural type is identity
    Given the welang program "structural-function.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Inline type annotation on function definition
    Given the welang program "inline-annotation.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Named type annotation on function definition
    Given the welang program "named-type-annotation.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Structural type call returns input value
    Given the welang program "structural-identity-value.we"
    When I compile and run it
    Then it should exit successfully
