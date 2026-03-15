Feature: Basic programs

  Scenario: Minimal program
    Given the welang program "hello.we"
    When I compile and run it
    Then it should exit successfully
