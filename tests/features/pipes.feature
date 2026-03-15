Feature: Pipe operator

  Scenario: Forward pipe operator
    Given the welang program "pipe.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Pipe execution order
    Given the welang program "pipe-order.we"
    When I compile and run it
    Then it should exit successfully
