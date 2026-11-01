use super::*;

/// 只接受 `ssh://[user@]host[:port]/path` 与 `git@host:path` 两种 git over SSH
/// 形式；拒绝 `ext::`、`file://`、`http(s)://`、`-` 开头、空白、`..`、`%`、反斜杠。
pub fn validate_git_url(raw: &str) -> Result<GitUrl, RepositoryError> {
    let url = raw.trim();
    if url.is_empty() || url.len() > MAX_URL_LEN {
        return Err(RepositoryError::InvalidUrl);
    }
    if url.starts_with('-')
        || url.chars().any(char::is_whitespace)
        || url.contains("..")
        || url.contains('%')
        || url.contains('\\')
    {
        return Err(RepositoryError::InvalidUrl);
    }

    let (user, host, path) = if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/').ok_or(RepositoryError::InvalidUrl)?;
        let (user, host_port) = authority
            .rsplit_once('@')
            .map_or(("git", authority), |(u, h)| (u, h));
        let host = match host_port.rsplit_once(':') {
            Some((h, port)) => {
                if h.is_empty() || port.is_empty() || port.len() > 5 || !port.chars().all(|c| c.is_ascii_digit()) {
                    return Err(RepositoryError::InvalidUrl);
                }
                h
            }
            None => host_port,
        };
        (user, host, path)
    } else if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest.split_once(':').ok_or(RepositoryError::InvalidUrl)?;
        ("git", host, path)
    } else {
        return Err(RepositoryError::InvalidUrl);
    };

    if user.is_empty() || !user.chars().all(host_char) {
        return Err(RepositoryError::InvalidUrl);
    }
    if host.is_empty() || !host.chars().all(host_char) {
        return Err(RepositoryError::InvalidUrl);
    }
    if path.is_empty() || path.starts_with('/') || !path.chars().all(path_char) {
        return Err(RepositoryError::InvalidUrl);
    }
    if let Some(allowed) = allowed_hosts() {
        if !allowed.iter().any(|item| item == host) {
            return Err(RepositoryError::InvalidUrl);
        }
    }
    Ok(GitUrl {
        user: user.to_string(),
        host: host.to_string(),
        path: path.to_string(),
        normalized: url.to_string(),
    })
}


/// `AI_PM_REPO_ALLOWED_HOSTS`（逗号分隔）存在时只允许这些 host。
pub(crate) fn allowed_hosts() -> Option<Vec<String>> {
    let raw = std::env::var("AI_PM_REPO_ALLOWED_HOSTS").ok()?;
    let hosts: Vec<String> = raw
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    if hosts.is_empty() {
        None
    } else {
        Some(hosts)
    }
}


pub(crate) fn normalize_url(raw: &str) -> Result<String, RepositoryError> {
    validate_git_url(raw).map(|url| url.normalized)
}


pub(crate) fn host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}


pub(crate) fn path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '~' | '/')
}

// ---------------------------------------------------------------- key + crypto


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_accepts_scp_and_ssh_forms() {
        assert!(normalize_url("git@gitlab.local:team/world-map.git").is_ok());
        assert!(normalize_url("ssh://git@gitlab.local:2222/team/repo.git").is_ok());
        assert!(normalize_url("ssh://gitlab.local/team/repo.git").is_ok());
    }

    #[test]
    fn url_rejects_executables_and_traversal() {
        for bad in [
            "ext::sh -c rm",
            "file:///etc/passwd",
            "https://github.com/team/repo.git",
            "-oProxyCommand=touch /tmp/pwned",
            "git@host:../escape",
            "git@host:team/repo with space.git",
            "git@host:team/repo%.git",
            "git@host:team\\repo.git",
        ] {
            assert!(normalize_url(bad).is_err(), "expected reject: {bad}");
        }
    }

    #[test]
    fn role_whitelist_is_enforced() {
        assert!(normalize_role("main").is_ok());
        assert!(normalize_role("docs").is_ok());
        assert!(normalize_role("unknown").is_err());
    }
}

