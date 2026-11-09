//! Production [`CapabilityProjection`] — the read-only capability surface the
//! task decomposition draws on (design §7.1 source two: "PC/KB 能力面").
//!
//! DK-D shipped only [`EmptyCapabilityProjection`]; DK-D's acceptance record
//! §九 assigns the real projection to DK-F. This module implements it:
//!
//! * **PC objects** (`pc.cognition_object` + its latest revision summary) —
//!   what the platform already believes about the product;
//! * **KB digests** (`kb.document` with `doc_key LIKE 'repo-%'` + the latest
//!   revision's change summary) — what the repository analysis produced.
//!
//! Read-only and **credential-free by construction**: the projection only
//! reads the four columns below and never touches provider configuration,
//! credentials or secrets. That is the same discipline D1 requires of the
//! injected context (the injection side additionally scans; here there is
//! nothing to scan because nothing else is selected).

use async_trait::async_trait;
use uuid::Uuid;

use crate::discovery_writeback::{
    CapabilityDigest, CapabilityFacts, CapabilityObject, CapabilityProjection, WriteBackError,
};

/// Max PC objects projected (the decomposition prompt is budgeted).
const MAX_OBJECTS: usize = 40;
/// Max KB digests projected.
const MAX_DIGESTS: usize = 10;
/// Max summary length per row.
const MAX_SUMMARY_CHARS: usize = 400;

const SQL_OBJECTS: &str = "SELECT o.object_type, \
              COALESCE(o.object_key, o.id::text) AS object_key, \
              COALESCE(rev.change_summary, o.display_name) AS summary \
         FROM pc.cognition_object o \
         LEFT JOIN LATERAL ( \
              SELECT change_summary FROM pc.cognition_revision cr \
               WHERE cr.tenant_id = o.tenant_id AND cr.project_id = o.project_id \
                 AND cr.object_id = o.id \
               ORDER BY cr.revision_no DESC LIMIT 1 \
         ) rev ON TRUE \
        WHERE o.project_id = $1 AND o.lifecycle_status <> 'deprecated' \
        ORDER BY o.created_at ASC, o.id \
        LIMIT $2";

const SQL_DIGESTS: &str = "SELECT d.doc_key, \
              COALESCE(r.change_summary, left(r.content, $3::int)) AS summary \
         FROM kb.document d \
         JOIN LATERAL ( \
              SELECT change_summary, content FROM kb.document_revision rev \
               WHERE rev.tenant_id = d.tenant_id AND rev.document_id = d.id \
               ORDER BY rev.revision_no DESC LIMIT 1 \
         ) r ON TRUE \
        WHERE d.project_id = $1 AND d.doc_key LIKE 'repo-%' \
        ORDER BY d.updated_at DESC, d.id \
        LIMIT $2";

/// The PG-backed capability projection.
pub struct PgCapabilityProjection {
    pool: sqlx::PgPool,
}

impl PgCapabilityProjection {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

fn cap(text: &str) -> String {
    if text.chars().count() <= MAX_SUMMARY_CHARS {
        return text.trim().to_string();
    }
    text.chars().take(MAX_SUMMARY_CHARS).collect::<String>()
}

#[async_trait]
impl CapabilityProjection for PgCapabilityProjection {
    async fn project(&self, project_id: Uuid) -> Result<CapabilityFacts, WriteBackError> {
        let object_rows: Vec<(String, String, String)> = sqlx::query_as(SQL_OBJECTS)
            .bind(project_id)
            .bind(MAX_OBJECTS as i64)
            .fetch_all(&self.pool)
            .await?;
        let digest_rows: Vec<(String, String)> = sqlx::query_as(SQL_DIGESTS)
            .bind(project_id)
            .bind(MAX_DIGESTS as i64)
            .bind(MAX_SUMMARY_CHARS as i64)
            .fetch_all(&self.pool)
            .await?;

        Ok(CapabilityFacts {
            objects: object_rows
                .into_iter()
                .map(|(object_type, object_key, summary)| CapabilityObject {
                    object_key,
                    object_type,
                    summary: cap(&summary),
                })
                .collect(),
            digests: digest_rows
                .into_iter()
                .map(|(doc_key, summary)| CapabilityDigest {
                    doc_key,
                    summary: cap(&summary),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_capped() {
        let long = "x".repeat(MAX_SUMMARY_CHARS + 50);
        assert_eq!(cap(&long).chars().count(), MAX_SUMMARY_CHARS);
    }

    #[test]
    fn projection_sql_is_read_only() {
        for sql in [SQL_OBJECTS, SQL_DIGESTS] {
            let lower = sql.to_ascii_lowercase();
            for forbidden in [
                "insert ",
                "update ",
                "delete ",
                "truncate ",
                "api_key",
                "credential",
                "secret",
                "password",
            ] {
                assert!(
                    !lower.contains(forbidden),
                    "capability projection SQL must not contain `{forbidden}`"
                );
            }
        }
    }
}
