## User Story: Default AI Behavior with Opt-Out Option

### Story

As a `gitie` user, I want the AI functionality to be enabled by default when I run commands, with the option to disable it using `--noai`, so that I can use the AI features more conveniently without having to specify `--ai` every time.

### Background

Currently, users need to explicitly add the `--ai` flag to use the AI capabilities of `gitie`. However, our analytics show that most users install `gitie` specifically for its AI features, making this extra flag redundant in most cases.

### Acceptance Criteria

1. When a user runs `gitie` without any AI-related flags, the AI functionality should be enabled by default
2. A new `--noai` flag should be implemented to explicitly disable AI functionality
3. The existing `--ai` flag should continue to work for backward compatibility
4. All documentation should be updated to reflect this change in default behavior
5. Help text should clearly explain the new default behavior and the `--noai` option

### Implementation Tasks

1. Modify the argument parsing logic in `cli.rs` to introduce the `--noai` flag
2. Update the main runtime logic in `main.rs` to:
   - Default to AI behavior unless `--noai` is present
   - Remove `--noai` from arguments before further processing
   - Maintain backward compatibility with the `--ai` flag
3. Update the commit command handler in `commit_commands.rs` to align with the new default behavior
4. Update README.md and other documentation to reflect the new default behavior
5. Add appropriate log messages for when AI features are used by default vs. explicitly disabled

### Testing Criteria

1. Verify that running `gitie commit` (without flags) generates an AI commit message
2. Confirm that `gitie status` provides AI explanation by default
3. Ensure that `gitie commit --noai` performs a standard git commit without AI
4. Verify that `gitie status --noai` performs a standard git status without AI explanation
5. Check that the existing `--ai` flag still works as expected for backward compatibility
6. Test combination of flags like `gitie commit --noai --ai` to ensure proper precedence

### Technical Notes

- The `--noai` flag should take precedence if both `--ai` and `--noai` are specified
- Care should be taken to maintain the same behavior for all subcommands and help text interactions
- Log appropriate debug information when the default AI behavior is activated