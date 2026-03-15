Feature: Arithmetic and comparison operators

  Scenario: multiply returns the product of two numbers
    Given the welang expression "(multiply [2, 5])"
    Then it should evaluate to 10

  Scenario: divide returns the integer quotient
    Given the welang expression "(divide [10, 2])"
    Then it should evaluate to 5

  Scenario: add returns the sum
    Given the welang expression "(add [4, 1])"
    Then it should evaluate to 5

  Scenario: subtract returns the difference
    Given the welang expression "(subtract [5, 1])"
    Then it should evaluate to 4

  Scenario: lessThan returns 1 when left is strictly less than right
    Given the welang expression "(lessThan [3, 5])"
    Then it should evaluate to 1

  Scenario: lessThan returns 0 when left is greater than right
    Given the welang expression "(lessThan [5, 3])"
    Then it should evaluate to 0

  Scenario: greaterThan returns 1 when left is strictly greater than right
    Given the welang expression "(greaterThan [5, 3])"
    Then it should evaluate to 1

  Scenario: greaterThan returns 0 when left is less than right
    Given the welang expression "(greaterThan [3, 5])"
    Then it should evaluate to 0

  Scenario: equal returns 1 for equal values
    Given the welang expression "(equal [5, 5])"
    Then it should evaluate to 1

  Scenario: equal returns 0 for unequal values
    Given the welang expression "(equal [5, 6])"
    Then it should evaluate to 0

  Scenario: lessThanOrEqual returns 1 when left is strictly less than right
    Given the welang expression "(lessThanOrEqual [3, 5])"
    Then it should evaluate to 1

  Scenario: lessThanOrEqual returns 1 for equal values
    Given the welang expression "(lessThanOrEqual [5, 5])"
    Then it should evaluate to 1

  Scenario: lessThanOrEqual returns 0 when left is greater than right
    Given the welang expression "(lessThanOrEqual [6, 5])"
    Then it should evaluate to 0

  Scenario: greaterThanOrEqual returns 1 when left is strictly greater than right
    Given the welang expression "(greaterThanOrEqual [5, 3])"
    Then it should evaluate to 1

  Scenario: greaterThanOrEqual returns 1 for equal values
    Given the welang expression "(greaterThanOrEqual [5, 5])"
    Then it should evaluate to 1

  Scenario: greaterThanOrEqual returns 0 when left is less than right
    Given the welang expression "(greaterThanOrEqual [3, 5])"
    Then it should evaluate to 0

  Scenario: float literal with zero exponent equals the integer part
    Given the welang expression "3f0"
    Then it should evaluate to 3

  Scenario: float literal with large exponent truncates to integer part
    Given the welang expression "3f14"
    Then it should evaluate to 3

  Scenario: float literal 1f9 truncates fractional part to 1
    Given the welang expression "1f9"
    Then it should evaluate to 1

  Scenario: arithmetic on float literals uses truncated integer values
    Given the welang expression "(multiply [3f0, 2f0])"
    Then it should evaluate to 6
