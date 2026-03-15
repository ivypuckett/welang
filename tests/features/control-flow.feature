Feature: Control flow

  Scenario: conditional takes the first matching arm for negative input
    Given the welang definitions:
      """
      abs: {(lessThan [x, 0]): (subtract [0, x]), _: x}
      """
    Then calling "abs" with -7 should return 7

  Scenario: conditional falls through to wildcard for non-negative input
    Given the welang definitions:
      """
      abs: {(lessThan [x, 0]): (subtract [0, x]), _: x}
      """
    Then calling "abs" with 3 should return 3

  Scenario: multi-arm conditional clamps value above range to upper bound
    Given the welang definitions:
      """
      clamp10: {(lessThan [x, 0]): 0, (greaterThan [x, 10]): 10, _: x}
      """
    Then calling "clamp10" with 15 should return 10

  Scenario: multi-arm conditional clamps value below range to lower bound
    Given the welang definitions:
      """
      clamp10: {(lessThan [x, 0]): 0, (greaterThan [x, 10]): 10, _: x}
      """
    Then calling "clamp10" with -3 should return 0

  Scenario: multi-arm conditional passes through in-range value unchanged
    Given the welang definitions:
      """
      clamp10: {(lessThan [x, 0]): 0, (greaterThan [x, 10]): 10, _: x}
      """
    Then calling "clamp10" with 5 should return 5

  Scenario: wildcard arm is taken when no condition matches for negative input
    Given the welang definitions:
      """
      clamp-positive: {(greaterThan [x, 0]): x, _: 0}
      """
    Then calling "clamp-positive" with -5 should return 0

  Scenario: first matching arm is taken over wildcard for positive input
    Given the welang definitions:
      """
      clamp-positive: {(greaterThan [x, 0]): x, _: 0}
      """
    Then calling "clamp-positive" with 3 should return 3

  Scenario: positive-only returns 0 for negative input
    Given the welang definitions:
      """
      positive-only: {(greaterThan [x, 0]): x, _: 0}
      """
    Then calling "positive-only" with -3 should return 0

  Scenario: positive-only returns the value unchanged for positive input
    Given the welang definitions:
      """
      positive-only: {(greaterThan [x, 0]): x, _: 0}
      """
    Then calling "positive-only" with 5 should return 5

  Scenario: rename binding gives the implicit parameter an explicit name
    Given the welang definitions:
      """
      abs-named: (n: {(lessThan [n, 0]): (subtract [0, n]), _: n})
      """
    Then calling "abs-named" with -9 should return 9

  Scenario: rename binding works correctly for positive input too
    Given the welang definitions:
      """
      abs-named: (n: {(lessThan [n, 0]): (subtract [0, n]), _: n})
      """
    Then calling "abs-named" with 5 should return 5
