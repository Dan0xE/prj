use super::cache::VcsInfo;
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

    let repo = gix::open(path).or(Err(FailedToOpen))?;
    let mut head = repo.head().or(Err(FailedToReadHead))?;

    let current_branch_name = match head.referent_name() {
        Some(name) => name.shorten().to_string(),
        None => "HEAD".to_string(),
    };

    let last_commit_summary = if head.is_unborn() {
        "No commits yet".to_string()
    } else {
        head.peel_to_commit()
            .or(Err(FailedToReadHead))?
            .message()
            .or(Err(FailedToReadHead))?
            .summary()
            .to_string()
    };

    let vcs_info = VcsInfo {
        last_commit_summary,
        current_branch_name,
        uncommitted_changes: None,
    };
    Ok(vcs_info)
}
