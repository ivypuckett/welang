Feature: Functions

  Scenario: Function composition
    Given the welang program "globals.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Rename binding
    Given the welang program "rename.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Recursive functions
    Given the welang program "recursion.we"
    When I compile and run it
    Then it should exit successfully
