use super::*;

pub(crate) fn commit_matches(materials: &[RepoMaterial], repo_id: &str, reference: &str) -> bool {
    let reference = reference.trim().to_ascii_lowercase();
    if reference.len() < 7
        || reference.len() > 40
        || !reference.chars().all(|c| c.is_ascii_hexdigit())
    {
        return false;
    }
    materials.iter().any(|material| {
        material.repository_id == repo_id
            && material
                .facts
                .commits
                .iter()
                .any(|commit| commit.sha.to_ascii_lowercase().starts_with(&reference))
    })
}

pub(crate) fn doc_matches(materials: &[RepoMaterial], repo_id: &str, reference: &str) -> bool {
    materials.iter().any(|material| {
        material.repository_id == repo_id
            && material.facts.docs.iter().any(|doc| doc.path == reference)
    })
}

pub(crate) fn evidence_grounded(evidence: &Evidence, materials: &[RepoMaterial]) -> bool {
    match evidence.kind {
        EvidenceKind::Doc => doc_matches(materials, &evidence.repo_id, &evidence.r#ref),
        EvidenceKind::Commit => commit_matches(materials, &evidence.repo_id, &evidence.r#ref),
    }
}

/// 证据接地：无真实 doc 路径或 commit sha 支撑的需求/任务被丢弃并计数。
pub fn ground_draft(draft: &ProjectDraft, materials: &[RepoMaterial]) -> (ProjectDraft, usize) {
    let mut grounded = draft.clone();
    let before = grounded.requirements.len() + grounded.tasks.len();

    grounded.requirements.retain(|requirement| {
        requirement
            .evidence
            .iter()
            .any(|evidence| evidence_grounded(evidence, materials))
    });
    grounded.tasks.retain(|task| {
        task.evidence
            .iter()
            .any(|evidence| evidence_grounded(evidence, materials))
    });
    for risk in &mut grounded.risks {
        risk.evidence
            .retain(|evidence| evidence_grounded(evidence, materials));
    }

    // 标题去重必须在接地之后：被丢掉的项不应占位。
    //
    // 平台以「项目 + 标题」为任务身份——执行器 `create_tasks` 遇同标题即视为
    // 同一任务而**合并**（`created=false`）。草案若带重复标题，落地时被静默合并：
    // 真实仓库的 commit subject 天然重复（ai-pm 自身 300 条 commit 只有 293 个
    // 不同 subject），合并后提案宣称的条目数与实际落地数不符，且无任何提示。
    // 按证据引用（commit sha / doc 路径）消歧，保证「一条 commit 一条任务」。
    let mut seen = std::collections::HashSet::new();
    {
        let mut titles: Vec<String> = grounded
            .requirements
            .iter()
            .map(|item| item.title.clone())
            .collect();
        let refs: Vec<String> = grounded
            .requirements
            .iter()
            .map(|item| first_ref(&item.evidence))
            .collect();
        uniquify(&mut titles, &refs, &mut seen);
        for (item, title) in grounded.requirements.iter_mut().zip(titles) {
            item.title = title;
        }
    }
    {
        let mut titles: Vec<String> = grounded
            .tasks
            .iter()
            .map(|item| item.title.clone())
            .collect();
        let refs: Vec<String> = grounded
            .tasks
            .iter()
            .map(|item| first_ref(&item.evidence))
            .collect();
        uniquify(&mut titles, &refs, &mut seen);
        for (item, title) in grounded.tasks.iter_mut().zip(titles) {
            item.title = title;
        }
    }

    let after = grounded.requirements.len() + grounded.tasks.len();
    (grounded, before - after)
}

/// 条目的首个证据引用（commit sha 或 doc 路径）——用作标题消歧的后缀。
fn first_ref(evidence: &[Evidence]) -> String {
    evidence
        .first()
        .map(|ev| ev.r#ref.clone())
        .unwrap_or_default()
}

/// 同标题时的消歧后缀：sha 取前 7 位，路径取末段文件名。
fn short_ref(reference: &str) -> String {
    let trimmed = reference.trim();
    if trimmed.len() >= 7 && trimmed.len() <= 40 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return trimmed[..7].to_string();
    }
    trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// 就地把重复标题改写成唯一值（追加消歧后缀，必要时再加序号）。
fn uniquify(titles: &mut [String], refs: &[String], seen: &mut std::collections::HashSet<String>) {
    for (index, title) in titles.iter_mut().enumerate() {
        let base = title.clone();
        let mut candidate = base.clone();
        if seen.contains(&candidate) {
            let hint = refs.get(index).map(|r| short_ref(r)).unwrap_or_default();
            candidate = if hint.is_empty() {
                format!("{base} #2")
            } else {
                format!("{base} · {hint}")
            };
            let mut ordinal = 2;
            while seen.contains(&candidate) {
                candidate = if hint.is_empty() {
                    format!("{base} #{ordinal}")
                } else {
                    format!("{base} · {hint} #{ordinal}")
                };
                ordinal += 1;
            }
        }
        seen.insert(candidate.clone());
        *title = candidate;
    }
}
// ------------------------------------------------------------------ apply
