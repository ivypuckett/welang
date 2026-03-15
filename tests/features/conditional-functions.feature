Feature: Conditional function behavior

  # Requirements from conditionals.we: positive-only is defined but not
  # tested in main. abs-named uses rename binding but is not tested in main.

  Scenario: positive-only clamps negative input to zero
    Given the welang program "positive-only.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: positive-only passes through positive input unchanged
    Given the welang program "positive-only-pass.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: abs-named computes absolute value via rename binding
    Given the welang program "abs-named.we"
    When I compile and run it
    Then it should exit successfully
