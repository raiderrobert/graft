Feature: Design gaps identified from lifecycle analysis

  # Gap 1: No rollback/downgrade mechanism
  # When a user pins an older version in graft.toml and runs graft sync,
  # the file on disk is not reverted because sync skips existing files.

  Scenario: Rollback to an older version after a bad upgrade
    Given a project with a synced graft "lint" at version "v2.0.0"
    And the local file contains v2.0.0 content
    When I edit graft.toml to set "lint" version to "v1.0.0"
    And I run "graft sync --force"
    Then the local file should contain v1.0.0 content
    And "graft.lock" should contain "v1.0.0"

  Scenario: Sync without --force skips existing files even when version changed
    Given a project with a synced graft "lint" at version "v2.0.0"
    When I edit graft.toml to set "lint" version to "v1.0.0"
    And I run "graft sync"
    Then the local file should still contain v2.0.0 content
    And stdout should contain "skip"

  Scenario: Force sync overwrites locally modified file
    Given a project with a synced graft "lint" at version "v1.0.0"
    And I modify the grafted file "lint"
    When I edit graft.toml to set "lint" version to "v1.0.0"
    And I run "graft sync --force"
    Then the local file should contain the original v1.0.0 content
    And the local modifications should be gone

  # Gap 2: Unsynced manifest entries not detected by graft check
  # If a developer adds a graft to graft.toml but forgets to run graft sync,
  # graft check should catch the missing lockfile entry.

  Scenario: Check detects manifest entry with no lockfile entry
    Given an empty project directory
    And a file "graft.toml" with content:
      """
      [deps.lint]
      source = "gh:org/configs/lint.yml"
      version = "v1.0.0"
      dest = ".github/workflows/lint.yml"
      """
    And no "graft.lock" file exists
    When I run "graft check"
    Then the exit code should be 1
    And stderr should contain "not synced"

  Scenario: Check detects partially synced manifest
    Given a project with a synced graft "lint" at version "v1.0.0"
    And I manually add a new entry "prettier" to graft.toml
    And I do not run graft sync
    When I run "graft check"
    Then the exit code should be 1
    And stdout should contain "prettier"
    And stdout should contain "not synced"

  Scenario: Check passes when all manifest entries are synced
    Given a project with a synced graft "lint" at version "v1.0.0"
    When I run "graft check"
    Then the exit code should be 0

  # Gap 3: No --dry-run on graft add
  # Users want to preview what they'd get before committing to a dependency.

  Scenario: Dry run add shows file content without writing anything
    Given an empty project directory
    When I run "graft add gh:org/configs/lint.yml@v1.0.0 --dry-run"
    Then the command should succeed
    And stdout should contain the upstream file content
    And a file "graft.toml" should not exist
    And a file "graft.lock" should not exist
    And a file "lint.yml" should not exist

  Scenario: Dry run add with explicit dest
    Given an empty project directory
    When I run "graft add gh:org/configs/lint.yml@v1.0.0 .github/workflows/lint.yml --dry-run"
    Then the command should succeed
    And a file ".github/workflows/lint.yml" should not exist

  Scenario: Dry run add when dest already exists
    Given an empty project directory
    And a file "lint.yml" with content "local version"
    When I run "graft add gh:org/configs/lint.yml@v1.0.0 --dry-run"
    Then the command should succeed
    And stdout should show the diff between local and upstream
    And "lint.yml" should contain "local version"

  # Gap 4: No --files flag on graft add CLI
  # The manifest supports a files field for directory grafts,
  # but the CLI has no way to pass it during graft add.

  Scenario: Add a directory graft with file filter
    Given an empty project directory
    When I run "graft add gh:org/configs/skills/@v1.0.0 .claude/skills/ --files brainstorming.md,debugging.md"
    Then the command should succeed
    And a file ".claude/skills/brainstorming.md" should exist
    And a file ".claude/skills/debugging.md" should exist
    And a file ".claude/skills/tdd.md" should not exist
    And "graft.toml" should contain "brainstorming.md"
    And "graft.toml" should contain "debugging.md"

  Scenario: Add a directory graft without file filter gets all files
    Given an empty project directory
    When I run "graft add gh:org/configs/skills/@v1.0.0 .claude/skills/"
    Then the command should succeed
    And a file ".claude/skills/brainstorming.md" should exist
    And a file ".claude/skills/debugging.md" should exist
    And a file ".claude/skills/tdd.md" should exist

  # Gap 5: No force-push tag warning
  # When an upstream tag is force-pushed to a different commit,
  # graft upgrade silently uses the new content without alerting.

  Scenario: Upgrade warns when tag resolves to a different commit than lockfile
    Given a project with a synced graft "lint" at version "v1.0.0"
    And the lockfile records commit "abc123" for "lint"
    And upstream tag "v1.0.0" has been force-pushed to commit "def456"
    When I run "graft upgrade lint"
    Then stdout should contain "warning"
    And stdout should contain "tag v1.0.0 now points to a different commit"
    And stdout should contain "abc123"
    And stdout should contain "def456"

  Scenario: Upgrade with force-pushed tag still shows the diff for review
    Given a project with a synced graft "lint" at version "v1.0.0"
    And upstream tag "v1.0.0" has been force-pushed with different content
    When I run "graft upgrade lint --dry-run"
    Then stdout should show the content difference
    And no files should be modified

  # Additional gap scenarios from quality review

  # Network failure during upgrade
  Scenario: Network failure during upgrade leaves all files untouched
    Given a project with synced grafts "lint" and "prettier"
    And the network is unavailable
    When I run "graft upgrade"
    Then the command should fail
    And the local file for "lint" should be unchanged
    And the local file for "prettier" should be unchanged
    And "graft.lock" should be unchanged

  # Per-graft error handling (not all-or-nothing)
  Scenario: One graft failing to upgrade does not block others
    Given a project with synced grafts "lint" at "v1.0.0" and "prettier" at "v1.0.0"
    And upstream has "lint" v1.1.0 available
    And upstream has "prettier" v1.1.0 available
    And the source path for "lint" no longer exists at v1.1.0
    When I run "graft upgrade"
    Then "prettier" should be upgraded to v1.1.0
    And stdout should contain an error for "lint"
    And stdout should contain "path not found"
    And the local file for "lint" should be unchanged

  # Malformed graft.toml
  Scenario: Malformed graft.toml produces a clear error
    Given an empty project directory
    And a file "graft.toml" with content "this is not valid toml {{{"
    When I run "graft list"
    Then the command should fail
    And stderr should contain "Failed to parse graft.toml"

  # Merge conflict in graft.toml itself
  Scenario: graft.toml with git merge conflict markers produces a clear error
    Given an empty project directory
    And a file "graft.toml" with content:
      """
      [deps.lint]
      source = "gh:org/configs/lint.yml"
      <<<<<<< HEAD
      version = "v1.0.0"
      =======
      version = "v2.0.0"
      >>>>>>> feature-branch
      dest = "lint.yml"
      """
    When I run "graft list"
    Then the command should fail
    And stderr should contain "Failed to parse graft.toml"

  # Directory graft where upstream removes a file
  Scenario: Directory upgrade warns about files removed upstream
    Given a project with a synced directory graft "skills" containing "a.md", "b.md", "c.md"
    And upstream version v2.0.0 no longer contains "c.md"
    When I run "graft upgrade skills"
    Then "a.md" and "b.md" should be upgraded
    And stdout should contain a warning that "c.md" was removed upstream
    And "c.md" should still exist locally

  # Auth failure
  Scenario: Clear error when authentication fails for private repo
    Given an empty project directory
    And no GitHub credentials are configured
    When I run "graft add gh:private-org/private-repo/config.yml@v1.0.0"
    Then the command should fail
    And stderr should contain "Could not authenticate"
    And stderr should contain "GH_TOKEN"
    And stderr should contain "gh auth login"
