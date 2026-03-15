Feature: Resolve conflicts after upgrade

  Scenario: Resolve nonexistent graft fails
    Given an empty project directory with "graft.toml"
    When I run "graft resolve nonexistent"
    Then the command should fail
