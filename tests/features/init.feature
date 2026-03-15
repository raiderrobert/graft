Feature: Initialize a project with graft

  Scenario: Create a new graft.toml in an empty directory
    Given an empty project directory
    When I run "graft init"
    Then the command should succeed
    And a file "graft.toml" should exist
    And "graft.toml" should contain "# Graft"

  Scenario: Init is idempotent when graft.toml already exists
    Given an empty project directory
    And a file "graft.toml" with content "# my existing config"
    When I run "graft init"
    Then the command should succeed
    And "graft.toml" should contain "# my existing config"
