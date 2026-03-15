Feature: Nominal type runtime semantics

  # Requirements from nominal-types.we: many nominal types are defined but
  # only (specialInt 0) is tested in main.

  Scenario: Nominal constructor preserves non-zero value
    Given the welang program "nominal-nonzero.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Named annotation with nominal type
    Given the welang program "nominal-named-annotation.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Nominal generic type acts as identity
    Given the welang program "nominal-generic-runtime.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Structural annotation referencing nominal generic
    Given the welang program "structural-over-nominal.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Nominal annotation referencing nominal generic
    Given the welang program "nominal-over-nominal.we"
    When I compile and run it
    Then it should exit successfully
