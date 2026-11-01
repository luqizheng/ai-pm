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

    let after = grounded.requirements.len() + grounded.tasks.len();
    (grounded, before - after)
}
// ------------------------------------------------------------------ apply


