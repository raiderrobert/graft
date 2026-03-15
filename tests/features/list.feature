Feature: List grafts and their status

  Scenario: List with no grafts
    Given an empty project directory with "graft.toml"
    When I run "graft list"
    Then the command should succeed
