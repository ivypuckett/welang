Feature: Basic programs

  Scenario: entry point returning 0 exits with code 0
    Given the welang expression "0"
    Then it should evaluate to 0

  Scenario: true literal compiles to integer 1
    Given the welang expression "true"
    Then it should evaluate to 1

  Scenario: false literal compiles to integer 0
    Given the welang expression "false"
    Then it should evaluate to 0

  Scenario: true is truthy and taken as a conditional arm
    Given the welang expression "{(true): 1, _: 0}"
    Then it should evaluate to 1

  Scenario: false is falsy and falls through to wildcard arm
    Given the welang expression "{(false): 1, _: 0}"
    Then it should evaluate to 0

  Scenario: boolean passed as argument to a function
    Given the welang definitions:
      """
      bool-result: {(x): 1, _: 0}
      """
    Then calling "bool-result" with 1 should return 1

  Scenario: false passed as argument evaluates as 0
    Given the welang definitions:
      """
      bool-result: {(x): 1, _: 0}
      """
    Then calling "bool-result" with 0 should return 0
