Feature: Check for outdated grafts

  Scenario: Outdated with no grafts
    Given an empty project directory with "graft.toml"
    When I run "graft outdated"
    Then the command should succeed
