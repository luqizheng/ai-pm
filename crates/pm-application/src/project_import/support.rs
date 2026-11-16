use super::*;

pub(crate) fn model_error_code(err: &ModelError) -> (&'static str, String) {
    match err {
        ModelError::Timeout => ("model_timeout", err.to_string()),
        ModelError::Unavailable(reason) => ("model_unavailable", reason.clone()),
        ModelError::BadOutput(reason) => ("bad_output", reason.clone()),
        ModelError::Configuration(reason) => ("configuration", reason.clone()),
    }
}

pub(crate) fn git_error_code(err: &git_access::GitError) -> (&'static str, String) {
    use git_access::GitError;
    // DEF-CHAIN-02：稳定错误码单源 = `GitError::stable_code`（含 network 类）；
    // detail 保持既有语义——Other/Io/Network 只回原文，不带 error 前缀。
    let detail = match err {
        GitError::Other(message) | GitError::Io(message) | GitError::Network(message) => {
            message.clone()
        }
        _ => err.to_string(),
    };
    (err.stable_code(), detail)
}
