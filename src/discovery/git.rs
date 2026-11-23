use super::cache::VcsInfo;
use git2::{ErrorCode, Repository};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to open repository")]
    FailedToOpen,
    #[error("failed to read HEAD")]
    FailedToReadHead,
}

pub fn fetch_vcs_info(path: &Path) -> Result<VcsInfo, GitError> {
    use GitError::*;

    let repo = Repository::open(path).or(Err(FailedToOpen))?;
    let head = match repo.head() {
        Ok(head) => head,
        Err(e) if e.code() == ErrorCode::UnbornBranch => {
            let head = repo.find_reference("HEAD").or(Err(FailedToReadHead))?;
            let branch = head
                .symbolic_target()
                .or(Err(FailedToReadHead))?
                .ok_or(FailedToReadHead)?;

            return Ok(VcsInfo {
                last_commit_summary: "No commits yet".to_string(),
                current_branch_name: branch.trim_start_matches("refs/heads/").to_string(),
                uncommitted_changes: None,
            });
        }
        Err(_) => return Err(FailedToReadHead),
    };

    let vcs_info = VcsInfo {
        last_commit_summary: head
            .peel_to_commit()
            .or(Err(FailedToReadHead))?
            .summary()
            .or(Err(FailedToReadHead))?
            .ok_or(FailedToReadHead)?
            .to_string(),

        current_branch_name: head.shorthand().or(Err(FailedToReadHead))?.to_string(),

        uncommitted_changes: None,
    };
    Ok(vcs_info)
}
