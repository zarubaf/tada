//! The counter of readable IDs such as `QST-001` (ADR 0038).

use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;

/// The next local number of `kind` in its scope: an event, or the organization (ADR 0038).
/// The counter changes in the transaction of the caller, so a rollback takes no number and a number is never given twice.
pub(crate) async fn next_local_number(
    conn: &mut PgConnection,
    scope: OrgScope,
    counter_scope: Uuid,
    kind: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"INSERT INTO local_id_counter (organization_id, scope_id, kind, next)
           VALUES ($1, $2, $3, 2)
           ON CONFLICT (organization_id, scope_id, kind) DO UPDATE SET next = local_id_counter.next + 1
           RETURNING next - 1 AS "number!""#,
        scope.organization_id().as_uuid(),
        counter_scope,
        kind,
    )
    .fetch_one(&mut *conn)
    .await
}
