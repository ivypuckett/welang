Feature: Control flow

  Scenario: Conditional expressions
    Given the welang program "conditionals.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Wildcard arm when no condition matches
    Given the welang program "if-no-else.we"
    When I compile and run it
    Then it should exit successfully
