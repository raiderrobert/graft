Feature: Upgrade grafts to newer versions

  Scenario: Upgrade with no grafts
    Given an empty project directory with "graft.toml"
    When I run "graft upgrade"
    Then the command should succeed
