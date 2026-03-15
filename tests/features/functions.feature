Feature: Functions

  Scenario: a function can call another function
    Given the welang definitions:
      """
      double: (multiply [2, x])
      quadruple: (double (double x))
      """
    Then calling "quadruple" with 3 should return 12

  Scenario: function composition chains results correctly
    Given the welang definitions:
      """
      double: (multiply [2, x])
      """
    Then calling "double" with 5 should return 10

  Scenario: rename binding allows using a named parameter in the body
    Given the welang definitions:
      """
      abs-named: (n: {(lessThan [n, 0]): (subtract [0, n]), _: n})
      """
    Then calling "abs-named" with -9 should return 9

  Scenario: rename binding works for non-negative input
    Given the welang definitions:
      """
      abs-named: (n: {(lessThan [n, 0]): (subtract [0, n]), _: n})
      """
    Then calling "abs-named" with 5 should return 5

  Scenario: factorial base case returns 1
    Given the welang definitions:
      """
      factorial:
        {(lessThanOrEqual [x, 1]):
          1,
        _: (multiply [x, (factorial (subtract [x, 1]))])}
      """
    Then calling "factorial" with 1 should return 1

  Scenario: factorial computes 5 factorial as 120
    Given the welang definitions:
      """
      factorial:
        {(lessThanOrEqual [x, 1]):
          1,
        _: (multiply [x, (factorial (subtract [x, 1]))])}
      """
    Then calling "factorial" with 5 should return 120

  Scenario: fibonacci base case for 0 returns 0
    Given the welang definitions:
      """
      fib:
        {(lessThan [x, 2]):
          x,
        _: (add [(fib (subtract [x, 1])), (fib (subtract [x, 2]))])}
      """
    Then calling "fib" with 0 should return 0

  Scenario: fibonacci base case for 1 returns 1
    Given the welang definitions:
      """
      fib:
        {(lessThan [x, 2]):
          x,
        _: (add [(fib (subtract [x, 1])), (fib (subtract [x, 2]))])}
      """
    Then calling "fib" with 1 should return 1

  Scenario: fibonacci computes fib(10) as 55
    Given the welang definitions:
      """
      fib:
        {(lessThan [x, 2]):
          x,
        _: (add [(fib (subtract [x, 1])), (fib (subtract [x, 2]))])}
      """
    Then calling "fib" with 10 should return 55
