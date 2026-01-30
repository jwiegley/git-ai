use crate::authorship::rebase_authorship::walk_commits_to_base;
use crate::commands::git_handlers::CommandHooksContext;
use crate::commands::hooks::commit_hooks::get_commit_default_author;
use crate::git::cli_parser::ParsedGitInvocation;
use crate::git::cli_parser::is_dry_run;
use crate::git::repository::Repository;
use crate::git::rewrite_log::RewriteLogEvent;
use crate::utils::debug_log;

pub fn pre_rebase_hook(
    parsed_args: &ParsedGitInvocation,
    repository: &mut Repository,
    command_hooks_context: &mut CommandHooksContext,
) {
    debug_log("=== REBASE PRE-COMMAND HOOK ===");

    // Check if we're continuing an existing rebase or starting a new one
    let rebase_dir = repository.path().join("rebase-merge");
    let rebase_apply_dir = repository.path().join("rebase-apply");
    let rebase_in_progress = rebase_dir.exists() || rebase_apply_dir.exists();

    debug_log(&format!(
        "Rebase directories check: rebase-merge={}, rebase-apply={}",
        rebase_dir.exists(),
        rebase_apply_dir.exists()
    ));

    // Check if there's an active Start event in the log that matches
    let has_active_start = has_active_rebase_start_event(repository);
    let is_continuing = rebase_in_progress && has_active_start;

    debug_log(&format!(
        "Rebase state: in_progress={}, has_active_start={}, is_continuing={}",
        rebase_in_progress, has_active_start, is_continuing
    ));

    if !is_continuing {
        // Starting a new rebase - capture original HEAD and log Start event
        if let Ok(head) = repository.head() {
            if let Ok(target) = head.target() {
                debug_log(&format!("Starting new rebase from HEAD: {}", target));
                command_hooks_context.rebase_original_head = Some(target.clone());

                // Determine if interactive
                let is_interactive = parsed_args.has_command_flag("-i")
                    || parsed_args.has_command_flag("--interactive");

                debug_log(&format!("Interactive rebase: {}", is_interactive));

                // Log the rebase start event
                let start_event = RewriteLogEvent::rebase_start(
                    crate::git::rewrite_log::RebaseStartEvent::new(target.clone(), is_interactive),
                );

                // Write to rewrite log
                match repository.storage.append_rewrite_event(start_event) {
                    Ok(_) => debug_log("✓ Logged RebaseStart event"),
                    Err(e) => debug_log(&format!("✗ Failed to log RebaseStart event: {}", e)),
                }
            }
        } else {
            debug_log("Could not read HEAD for new rebase");
        }
    } else {
        debug_log("Continuing existing rebase (will read original head from log in post-hook)");
    }
}

pub fn handle_rebase_post_command(
    context: &CommandHooksContext,
    parsed_args: &ParsedGitInvocation,
    exit_status: std::process::ExitStatus,
    repository: &mut Repository,
) {
    debug_log("=== REBASE POST-COMMAND HOOK ===");
    debug_log(&format!("Exit status: {}", exit_status));

    // Check if rebase is still in progress
    let rebase_dir = repository.path().join("rebase-merge");
    let rebase_apply_dir = repository.path().join("rebase-apply");
    let is_in_progress = rebase_dir.exists() || rebase_apply_dir.exists();

    debug_log(&format!(
        "Rebase directories check: rebase-merge={}, rebase-apply={}",
        rebase_dir.exists(),
        rebase_apply_dir.exists()
    ));

    if is_in_progress {
        // Rebase still in progress (conflict or not finished)
        debug_log("⏸ Rebase still in progress, waiting for completion (conflict or multi-step)");
        return;
    }

    if is_dry_run(&parsed_args.command_args) {
        debug_log("Skipping rebase post-hook for dry-run");
        return;
    }

    // Rebase is done (completed or aborted)
    // Try to find the original head from context OR from the rewrite log
    let original_head_from_context = context.rebase_original_head.clone();
    let original_head_from_log = find_rebase_start_event_original_head(repository);

    debug_log(&format!(
        "Original head: context={:?}, log={:?}",
        original_head_from_context, original_head_from_log
    ));

    let original_head = original_head_from_context.or(original_head_from_log);

    if !exit_status.success() {
        // Rebase was aborted or failed - log Abort event
        if let Some(orig_head) = original_head {
            debug_log(&format!("✗ Rebase aborted/failed from {}", orig_head));
            let abort_event = RewriteLogEvent::rebase_abort(
                crate::git::rewrite_log::RebaseAbortEvent::new(orig_head),
            );
            match repository.storage.append_rewrite_event(abort_event) {
                Ok(_) => debug_log("✓ Logged RebaseAbort event"),
                Err(e) => debug_log(&format!("✗ Failed to log RebaseAbort event: {}", e)),
            }
        } else {
            debug_log("✗ Rebase failed but couldn't determine original head");
        }
        return;
    }

    // Rebase completed successfully!
    debug_log("✓ Rebase completed successfully");
    if let Some(original_head) = original_head {
        debug_log(&format!(
            "Processing completed rebase from {}",
            original_head
        ));
        process_completed_rebase(repository, &original_head, parsed_args);
    } else {
        debug_log("⚠ Rebase completed but couldn't determine original head");
    }
}

/// Check if there's an active rebase Start event (not followed by Complete or Abort)
fn has_active_rebase_start_event(repository: &Repository) -> bool {
    let events = match repository.storage.read_rewrite_events() {
        Ok(events) => events,
        Err(_) => return false,
    };

    // Events are newest-first
    // If we find Complete or Abort before Start, there's no active rebase
    // If we find Start before Complete/Abort, there's an active rebase
    for event in events {
        match event {
            RewriteLogEvent::RebaseComplete { .. } | RewriteLogEvent::RebaseAbort { .. } => {
                return false; // Found completion/abort first, no active rebase
            }
            RewriteLogEvent::RebaseStart { .. } => {
                return true; // Found start first, active rebase
            }
            _ => continue,
        }
    }

    false // No rebase events found
}

/// Find the original head from the most recent Rebase Start event in the log
fn find_rebase_start_event_original_head(repository: &Repository) -> Option<String> {
    let events = repository.storage.read_rewrite_events().ok()?;

    // Find the most recent Start event (events are newest-first)
    for event in events {
        match event {
            RewriteLogEvent::RebaseStart { rebase_start } => {
                return Some(rebase_start.original_head.clone());
            }
            _ => continue,
        }
    }

    None
}

fn process_completed_rebase(
    repository: &mut Repository,
    original_head: &str,
    parsed_args: &ParsedGitInvocation,
) {
    debug_log(&format!(
        "--- Processing completed rebase from {} ---",
        original_head
    ));

    // Get the new HEAD
    let new_head = match repository.head() {
        Ok(head) => match head.target() {
            Ok(target) => {
                debug_log(&format!("New HEAD: {}", target));
                target
            }
            Err(e) => {
                debug_log(&format!("✗ Failed to get HEAD target: {}", e));
                return;
            }
        },
        Err(e) => {
            debug_log(&format!("✗ Failed to get HEAD: {}", e));
            return;
        }
    };

    // If HEAD didn't change, nothing to do
    if original_head == new_head {
        debug_log("Rebase resulted in no changes (fast-forward or empty)");
        return;
    }

    // Build commit mappings
    debug_log(&format!(
        "Building commit mappings: {} -> {}",
        original_head, new_head
    ));
    let (original_commits, new_commits) =
        match build_rebase_commit_mappings(repository, original_head, &new_head) {
            Ok(mappings) => {
                debug_log(&format!(
                    "✓ Built mappings: {} original commits -> {} new commits",
                    mappings.0.len(),
                    mappings.1.len()
                ));
                mappings
            }
            Err(e) => {
                debug_log(&format!("✗ Failed to build rebase mappings: {}", e));
                return;
            }
        };

    if original_commits.is_empty() {
        debug_log("No commits to rewrite authorship for");
        return;
    }

    debug_log(&format!("Original commits: {:?}", original_commits));
    debug_log(&format!("New commits: {:?}", new_commits));

    // Determine rebase type
    let is_interactive =
        parsed_args.has_command_flag("-i") || parsed_args.has_command_flag("--interactive");
    debug_log(&format!(
        "Rebase type: {}",
        if is_interactive {
            "interactive"
        } else {
            "normal"
        }
    ));

    let rebase_event =
        RewriteLogEvent::rebase_complete(crate::git::rewrite_log::RebaseCompleteEvent::new(
            original_head.to_string(),
            new_head.clone(),
            is_interactive,
            original_commits.clone(),
            new_commits.clone(),
        ));

    debug_log("Creating RebaseComplete event and rewriting authorship...");
    let commit_author = get_commit_default_author(repository, &parsed_args.command_args);

    repository.handle_rewrite_log_event(
        rebase_event,
        commit_author,
        false, // don't suppress output
        true,  // save to log
    );

    debug_log("✓ Rebase authorship rewrite complete");
}

pub(crate) fn build_rebase_commit_mappings(
    repository: &Repository,
    original_head: &str,
    new_head: &str,
) -> Result<(Vec<String>, Vec<String>), crate::error::GitAiError> {
    // Get commits from new_head and original_head
    let new_head_commit = repository.find_commit(new_head.to_string())?;
    let original_head_commit = repository.find_commit(original_head.to_string())?;

    // Find merge base between original and new
    let merge_base = repository.merge_base(original_head_commit.id(), new_head_commit.id())?;

    // Walk from original_head to merge_base to get the commits that were rebased
    let original_commits = walk_commits_to_base(repository, original_head, &merge_base)?;

    // Walk from new_head to merge_base to get the actual rebased commits
    // This correctly handles squashing, dropping, and other interactive rebase operations
    let new_commits = walk_commits_to_base(repository, new_head, &merge_base)?;

    // Reverse both so they're in chronological order (oldest first)
    let mut original_commits = original_commits;
    let mut new_commits = new_commits;
    original_commits.reverse();
    new_commits.reverse();

    debug_log(&format!(
        "Commit mapping: {} original -> {} new (merge_base: {})",
        original_commits.len(),
        new_commits.len(),
        merge_base
    ));

    // Always pass all commits through - let the authorship rewriting logic
    // handle many-to-one, one-to-one, and other mapping scenarios properly
    Ok((original_commits, new_commits))
}
