use super::*;

/// Applies a compensation spec inside the caller's transaction. Returns the
/// per-row before/after JSON for the rollback audit row, the last resulting
/// revision, and the inverse spec (undo-the-undo) when one exists.
pub(crate) async fn apply_compensation(
    tx: &mut Tx,
    spec: &CompensationSpec,
) -> Result<(Value, Value, i64, Option<CompensationSpec>), CommandError> {
    match spec {
        CompensationSpec::Restore(entries) => {
            let mut before_rows = Vec::with_capacity(entries.len());
            let mut after_rows = Vec::with_capacity(entries.len());
            let mut inverse_entries = Vec::with_capacity(entries.len());
            let mut resulting_revision = 0i64;
            for entry in entries {
                let (table, _, revcol) = entry.table.mapping();
                let (before, after, revision) = restore_entry(tx, entry).await?;
                before_rows.push(json!({ "table": table, "target_id": entry.target_id, "row": before }));
                after_rows.push(json!({ "table": table, "target_id": entry.target_id, "row": after }));
                // Undo-the-undo: restore the observed pre-rollback state,
                // guarded at the revision the rollback leaves behind.
                let mut inverse_fields = before
                    .as_object()
                    .expect("to_jsonb(t) always yields a JSON object")
                    .clone();
                inverse_fields.insert(revcol.to_string(), json!(revision));
                inverse_entries.push(SnapshotEntry {
                    table: entry.table,
                    target_id: entry.target_id,
                    fields: Value::Object(inverse_fields),
                });
                resulting_revision = revision;
            }
            Ok((
                Value::Array(before_rows),
                Value::Array(after_rows),
                resulting_revision,
                Some(CompensationSpec::Restore(inverse_entries)),
            ))
        }
        CompensationSpec::Delete { table, target_id } => {
            let (table_name, pk, _) = table.mapping();
            let result = sqlx::query(&format!("DELETE FROM {table_name} WHERE {pk} = $1"))
                .bind(target_id)
                .execute(&mut **tx)
                .await
                .map_err(db_err)?;
            if result.rows_affected() == 0 {
                return Err(CommandError::Validation(format!(
                    "compensation target row missing in {table_name}: {target_id}"
                )));
            }
            let after = json!({ "deleted": true, "table": table_name, "target_id": target_id });
            // A deleted row cannot be re-created from a snapshot spec; the
            // rollback audit row carries no inverse command (M1: no producer).
            Ok((Value::Null, after, 0, None))
        }
    }
}


/// Restores one snapshot entry under a fresh revision guard. Returns the
/// pre-restore row JSON, the post-restore row JSON and the new revision.
pub(crate) async fn restore_entry(
    tx: &mut Tx,
    entry: &SnapshotEntry,
) -> Result<(Value, Value, i64), CommandError> {
    let (table, pk, revcol) = entry.table.mapping();
    let fields = entry.fields.as_object().ok_or_else(|| {
        CommandError::Validation("snapshot entry `fields` must be a JSON object".to_string())
    })?;
    for key in fields.keys() {
        validate_identifier(key)?;
    }
    let expected = fields.get(revcol).and_then(Value::as_i64).ok_or_else(|| {
        CommandError::Validation(format!(
            "snapshot entry is missing its revision column `{revcol}`"
        ))
    })?;
    // JSON null fields mean SQL NULL for any column type: they are written as
    // NULL literals, not bound parameters — a TEXT-typed NULL parameter fails
    // to prepare against non-text columns (uuid, timestamptz, …).
    let (null_pairs, set_pairs): (Vec<_>, Vec<_>) = fields
        .iter()
        .filter(|(key, _)| key.as_str() != revcol)
        .partition(|(_, value)| value.is_null());
    let null_cols: Vec<&String> = null_pairs.into_iter().map(|(key, _)| key).collect();
    let set_cols: Vec<&String> = set_pairs.into_iter().map(|(key, _)| key).collect();
    if set_cols.is_empty() && null_cols.is_empty() {
        return Err(CommandError::Validation(
            "snapshot entry has no columns to restore".to_string(),
        ));
    }
    // Whole-row snapshot of the current state (before the restore), as JSON.
    let before: Value = sqlx::query_scalar(&format!(
        "SELECT to_jsonb(t) FROM {table} t WHERE t.{pk} = $1 FOR UPDATE"
    ))
    .bind(entry.target_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?
    .ok_or_else(|| {
        CommandError::Validation(format!(
            "compensation target row missing in {table}: {}",
            entry.target_id
        ))
    })?;
    let actual = before.get(revcol).and_then(Value::as_i64).ok_or_else(|| {
        CommandError::Validation(format!("row in {table} is missing revision column `{revcol}`"))
    })?;
    if actual != expected {
        return Err(CommandError::RevisionConflict {
            expected,
            actual,
        });
    }
    // String snapshots may target non-text columns (uuid ids, timestamptz
    // plans): a TEXT-typed parameter does not assignment-cast into them, so
    // those assignments get an explicit `::type` cast.
    let types = column_types(tx, table).await?;
    // Params: $1 = target id, $2 = expected revision, $3.. = column values.
    // Null columns come first (no parameters), then the value columns in order.
    let mut sql = format!("UPDATE {table} SET ");
    for (idx, col) in null_cols.iter().enumerate() {
        if idx > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("{col} = NULL"));
    }
    if !null_cols.is_empty() && !set_cols.is_empty() {
        sql.push_str(", ");
    }
    for (idx, col) in set_cols.iter().enumerate() {
        if idx > 0 {
            sql.push_str(", ");
        }
        match types.get(*col) {
            Some(data_type) if data_type != "text" && fields[*col].is_string() => {
                sql.push_str(&format!("{col} = ${}::{data_type}", idx + 3));
            }
            _ => sql.push_str(&format!("{col} = ${}", idx + 3)),
        }
    }
    sql.push_str(&format!(", {revcol} = $2 + 1 WHERE {pk} = $1"));
    let mut query = sqlx::query(&sql).bind(entry.target_id).bind(expected);
    for col in &set_cols {
        query = bind_field(query, &fields[*col]);
    }
    query.execute(&mut **tx).await.map_err(db_err)?;
    let resulting_revision = expected + 1;
    let mut after = entry.fields.clone();
    after[revcol] = json!(resulting_revision);
    Ok((before, after, resulting_revision))
}


/// Column data types for a schema-qualified table, read from the catalog.
/// Used by [`restore_entry`] to decide which SET assignments need an explicit
/// cast when replaying a stored snapshot.
pub(crate) async fn column_types(
    tx: &mut Tx,
    table: &str,
) -> Result<HashMap<String, String>, CommandError> {
    let (schema, name) = table.split_once('.').ok_or_else(|| {
        CommandError::Validation(format!("table `{table}` must be schema-qualified"))
    })?;
    let rows = sqlx::query(
        "SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = $2",
    )
    .bind(schema)
    .bind(name)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("column_name"),
                row.get::<String, _>("data_type"),
            )
        })
        .collect())
}


