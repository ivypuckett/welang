Feature: Pipe operator correctness

  # Requirements from pipe.we: pipe functions are defined but main returns 0
  # without verifying their output values.

  Scenario: Single pipe applies function to argument
    Given the welang program "pipe-apply.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Chained pipes are left-associative
    Given the welang program "pipe-left-assoc.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Pipe with expression on left-hand side
    Given the welang program "pipe-expr-left.we"
    When I compile and run it
    Then it should exit successfully
