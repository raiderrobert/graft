Feature: Remove a graft

  Scenario: Remove nonexistent graft fails
    Given an empty project directory with "graft.toml"
    When I run "graft remove nonexistent"
    Then the command should fail
