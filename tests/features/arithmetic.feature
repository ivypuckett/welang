Feature: Arithmetic operations

  Scenario: Four arithmetic operators
    Given the welang program "arithmetic.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Comparison operators
    Given the welang program "comparisons.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Float literals
    Given the welang program "floats.we"
    When I compile and run it
    Then it should exit successfully
