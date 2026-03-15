Feature: Map literals and field access

  Scenario: Map literals and dot field access
    Given the welang program "map.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Heterogeneous map values
    Given the welang program "heterogeneous-map.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Dot notation for operators
    Given the welang program "dot-operators.we"
    When I compile and run it
    Then it should exit successfully
