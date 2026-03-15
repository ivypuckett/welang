Feature: Pipe operator

  Scenario: single pipe applies the right-hand function to the left-hand value
    Given the welang definitions:
      """
      double: (multiply [2, x])
      """
    Given the welang expression "(3 | double)"
    Then it should evaluate to 6

  Scenario: chained pipes are left-associative
    Given the welang definitions:
      """
      double: (multiply [2, x])
      """
    Given the welang expression "(3 | double | double)"
    Then it should evaluate to 12

  Scenario: pipe with a sub-expression on the left-hand side
    Given the welang definitions:
      """
      double: (multiply [2, x])
      inc: (add [x, 1])
      """
    Given the welang expression "(double 3 | inc)"
    Then it should evaluate to 7

  Scenario: longer pipe chain with expression on left
    Given the welang definitions:
      """
      double: (multiply [2, x])
      inc: (add [x, 1])
      """
    Given the welang expression "(double 3 | double | inc)"
    Then it should evaluate to 13

  Scenario: pipeline applies ten functions in left-to-right order
    Given the welang definitions:
      """
      n1: (add [(multiply [2, x]), 1])
      n2: (add [(multiply [2, x]), 2])
      n3: (add [(multiply [2, x]), 3])
      n4: (add [(multiply [2, x]), 4])
      n5: (add [(multiply [2, x]), 5])
      n6: (add [(multiply [2, x]), 6])
      n7: (add [(multiply [2, x]), 7])
      n8: (add [(multiply [2, x]), 8])
      n9: (add [(multiply [2, x]), 9])
      n10: (add [(multiply [2, x]), 10])
      pipeline: (n3 n2 n1 x | n5 n4 | n6 | n10 n9 n8 n7)
      """
    Then calling "pipeline" with 0 should return 2036
