use super::*;

pub(crate) fn model_error_code(err: &ModelError) -> (&'static str, String) {
    match err {
        ModelError::Timeout => ("model_timeout", err.to_string()),
        ModelError::Unavailable(reason) => ("model_unavailable", reason.clone()),
        ModelError::BadOutput(reason) => ("bad_output", reason.clone()),
    }
}


pub(crate) fn git_error_code(err: &git_access::GitError) -> (&'static str, String) {
    use git_access::GitError;
    match err {
        GitError::Timeout => ("git_timeout", err.to_string()),
        GitError::AuthFailed => ("git_auth_failed", err.to_string()),
        GitError::NotFound => ("git_not_found", err.to_string()),
        GitError::TooLarge => ("repo_too_large", err.to_string()),
        GitError::Other(message) => ("git_failed", message.clone()),
        GitError::Io(message) => ("git_failed", message.clone()),
    }
}


