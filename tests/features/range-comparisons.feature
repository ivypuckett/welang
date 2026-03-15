Feature: Range comparison operators

  # Requirements from comparisons.we and dot-operators.we:
  # lessThanOrEqual and greaterThanOrEqual need dedicated tests.

  Scenario: lessThanOrEqual accepts strictly less than
    Given the welang program "leq-less.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: lessThanOrEqual accepts equal values
    Given the welang program "leq-equal.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: greaterThanOrEqual accepts strictly greater than
    Given the welang program "geq-greater.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: greaterThanOrEqual accepts equal values
    Given the welang program "geq-equal.we"
    When I compile and run it
    Then it should exit successfully
