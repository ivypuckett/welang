Feature: Fibonacci recursion

  # Requirements from recursion.we: fib is defined but not tested in main

  Scenario: Fibonacci base case zero
    Given the welang program "fib-zero.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Fibonacci base case one
    Given the welang program "fib-one.we"
    When I compile and run it
    Then it should exit successfully

  Scenario: Fibonacci recursive case
    Given the welang program "fib-recursive.we"
    When I compile and run it
    Then it should exit successfully
