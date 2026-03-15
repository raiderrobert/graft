Feature: Add a file dependency from a GitHub repo

  Scenario: Add refuses when destination already exists
    Given an empty project directory with "graft.toml"
    And a file "Makefile" with content "existing content"
    When I run "graft add gh:owner/repo/Makefile@v1.0.0 Makefile"
    Then the command should fail
    And stderr should contain "already exists"

  Scenario: Add rejects path traversal
    Given an empty project directory with "graft.toml"
    When I run "graft add gh:owner/repo/evil@v1.0.0 ../etc/passwd"
    Then the command should fail

  Scenario: Add rejects .git directory targets
    Given an empty project directory with "graft.toml"
    When I run "graft add gh:owner/repo/hook@v1.0.0 .git/hooks/pre-commit"
    Then the command should fail
