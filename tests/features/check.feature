Feature: Check graft status for CI

  Scenario: Check succeeds with no grafts
    Given an empty project directory with "graft.toml"
    When I run "graft check"
    Then the exit code should be 0
