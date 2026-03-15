Feature: Strings and printing

  Scenario: print with an integer argument returns the integer
    Given the welang expression "(print 42)"
    Then it should evaluate to 42

  Scenario: print with a string argument returns 0
    Given the welang expression "(print \"hello, world\")"
    Then it should evaluate to 0

  Scenario: first of multiple string literals returns 0 from print
    Given the welang expression "(print \"foo\")"
    Then it should evaluate to 0

  Scenario: second of multiple string literals returns 0 from print
    Given the welang expression "(print \"bar\")"
    Then it should evaluate to 0

  Scenario: third of multiple string literals returns 0 from print
    Given the welang expression "(print \"baz\")"
    Then it should evaluate to 0

  Scenario: multiple string literals in one program each return 0
    Given the welang definitions:
      """
      check-strings:
        {(equal [(print "foo"), 0]):
          {(equal [(print "bar"), 0]):
            {(equal [(print "baz"), 0]): 1, _: 0},
          _: 0},
        _: 0}
      """
    Then calling "check-strings" with 0 should return 1
