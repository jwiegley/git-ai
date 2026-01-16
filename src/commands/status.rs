use crate::authorship::stats::{CommitStats, write_stats_to_terminal};
use crate::authorship::working_log::CheckpointKind;
use crate::commands::checkpoint;
use crate::error::GitAiError;
use crate::git::find_repository;
use std::time::{SystemTime, UNIX_EPOCH};

struct CheckpointInfo {
    time_ago: String,
    additions: u32,
    deletions: u32,
    tool_model: String,
    is_human: bool,
}

pub fn handle_status(_args: &[String]) {
    if let Err(e) = run_status() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_status() -> Result<(), GitAiError> {
    let repo = find_repository(&vec![])?;

    // Get the current user name from git config for the human checkpoint
    let default_user_name = match repo.config_get_str("user.name") {
        Ok(Some(name)) if !name.trim().is_empty() => name,
        _ => "unknown".to_string(),
    };

    let _ = checkpoint::run(
        &repo,
        &default_user_name,
        CheckpointKind::Human,
        false, // show_working_log
        false, // reset
        true,  // quiet
        None,  // agent_run_result
        false, // is_pre_commit - don't skip if no AI checkpoints
    );

    let head = repo.head()?;
    let head_sha = head.target()?;

    let working_log = repo.storage.working_log_for_base_commit(&head_sha);
    let checkpoints = working_log.read_all_checkpoints()?;

    if checkpoints.is_empty() {
        eprintln!(
            "No checkpoints recorded since last commit ({})",
            &head_sha[..7]
        );
        eprintln!();

        eprintln!(
            "If you've made AI edits recently and don't see them here, you might need to install hooks:"
        );
        eprintln!();
        eprintln!("  git-ai install-hooks");
        eprintln!();
        return Ok(());
    }

    let mut human_additions = 0u32;
    let mut ai_additions = 0u32;
    let mut total_deletions = 0u32;

    let mut checkpoint_infos = Vec::new();

    for checkpoint in checkpoints.iter().rev() {
        let (additions, deletions) = (
            checkpoint.line_stats.additions,
            checkpoint.line_stats.deletions,
        );

        total_deletions += deletions;

        match checkpoint.kind {
            CheckpointKind::Human => {
                human_additions += additions;
            }
            CheckpointKind::AiAgent | CheckpointKind::AiTab => {
                ai_additions += additions;
            }
        }

        let tool_model = checkpoint
            .agent_id
            .as_ref()
            .map(|a| format!("{} {}", capitalize(&a.tool), &a.model))
            .unwrap_or_else(|| "Human".to_string());

        let is_human = checkpoint.kind == CheckpointKind::Human;
        checkpoint_infos.push(CheckpointInfo {
            time_ago: format_time_ago(checkpoint.timestamp),
            additions,
            deletions,
            tool_model,
            is_human,
        });
    }

    // Build CommitStats to reuse existing display function
    let stats = CommitStats {
        human_additions,
        ai_additions,
        ai_accepted: ai_additions, // For status, treat all AI as accepted
        git_diff_added_lines: human_additions + ai_additions,
        git_diff_deleted_lines: total_deletions,
        ..Default::default()
    };

    // Use existing stats display
    write_stats_to_terminal(&stats, true);

    // Print checkpoint list
    println!();
    for cp in &checkpoint_infos {
        let add_str = if cp.additions > 0 {
            format!("+{}", cp.additions)
        } else {
            "0".to_string()
        };
        let del_str = if cp.deletions > 0 {
            format!("-{}", cp.deletions)
        } else {
            "0".to_string()
        };

        let line = format!(
            "{:<14} {:>5}  {:>5}  {}",
            cp.time_ago, add_str, del_str, cp.tool_model
        );

        if cp.is_human {
            println!("\x1b[90m{}\x1b[0m", line);
        } else {
            println!("{}", line);
        }
    }

    Ok(())
}

fn format_time_ago(timestamp: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        format!("{} secs ago", diff)
    } else if diff < 3600 {
        format!("{} mins ago", diff / 60)
    } else if diff < 86400 {
        format!("{} hours ago", diff / 3600)
    } else {
        format!("{} days ago", diff / 86400)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
