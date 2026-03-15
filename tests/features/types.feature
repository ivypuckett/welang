Feature: Type system

  Scenario: Structural types
    Given the welang program "structural-types.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Nominal types
    Given the welang program "nominal-types.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Generic structural types
    Given the welang program "generics.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Generic type specialization
    Given the welang program "generic-specialization.we"
    When I compile and run it
    Then it should exit successfully
