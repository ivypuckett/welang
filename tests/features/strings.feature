Feature: Strings and printing

  Scenario: Print integer
    Given the welang program "print.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Print string literal
    Given the welang program "string-print.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Multiple string literals
    Given the welang program "multiple-strings.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Boolean literals
    Given the welang program "booleans.we"
    When I compile and run it
    Then it should exit successfully
