Feature: Map literals and field access

  Scenario: dot notation reads the first named field from a map literal
    Given the welang expression "{x: 10, y: 20}.x"
    Then it should evaluate to 10

  Scenario: dot notation reads the second named field from a map literal
    Given the welang expression "{x: 10, y: 20}.y"
    Then it should evaluate to 20

  Scenario: get built-in retrieves a field value by name
    Given the welang definitions:
      """
      getVal: (get [{label: "number", value: x}, value])
      """
    Then calling "getVal" with 5 should return 5

  Scenario: map values can be heterogeneous types
    Given the welang definitions:
      """
      getVal: (get [{label: "number", value: x}, value])
      """
    Then calling "getVal" with 42 should return 42

  Scenario: x.add y is dot notation for (add [x, y])
    Given the welang expression "5.add 3"
    Then it should evaluate to 8

  Scenario: x.subtract y is dot notation for (subtract [x, y])
    Given the welang expression "10.subtract 4"
    Then it should evaluate to 6

  Scenario: x.multiply y is dot notation for (multiply [x, y])
    Given the welang expression "3.multiply 4"
    Then it should evaluate to 12

  Scenario: x.divide y is dot notation for (divide [x, y])
    Given the welang expression "12.divide 4"
    Then it should evaluate to 3

  Scenario: x.lessThan y is dot notation for (lessThan [x, y])
    Given the welang expression "3.lessThan 5"
    Then it should evaluate to 1

  Scenario: x.greaterThan y is dot notation for (greaterThan [x, y])
    Given the welang expression "5.greaterThan 3"
    Then it should evaluate to 1

  Scenario: x.lessThanOrEqual y returns 1 for equal values
    Given the welang expression "5.lessThanOrEqual 5"
    Then it should evaluate to 1

  Scenario: x.greaterThanOrEqual y returns 1 for equal values
    Given the welang expression "5.greaterThanOrEqual 5"
    Then it should evaluate to 1
