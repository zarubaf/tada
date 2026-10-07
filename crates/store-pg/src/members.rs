//! Organization memberships and invitations (ADR 0056).

use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::domain::ids::OrganizationId;

/// Revokes the pending invitations `ids` of the organization and deletes their tokens.
/// A revoked invitation keeps no token, so no mailed or printed link of it works any more.
/// Returns the IDs of the invitations that were pending; the other IDs stay as they are.
pub(crate) async fn revoke_invitations(
    conn: &mut PgConnection,
    organization_id: OrganizationId,
    ids: &[Uuid],
    now: Timestamp,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let revoked = sqlx::query_scalar!(
        "UPDATE invitation SET status = 'revoked', revoked_at = $3
         WHERE organization_id = $1 AND id = ANY($2) AND status = 'pending'
         RETURNING id",
        organization_id.as_uuid(),
        ids,
        now.to_sqlx() as _,
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM invitation_token WHERE organization_id = $1 AND invitation_id = ANY($2)",
        organization_id.as_uuid(),
        &revoked,
    )
    .execute(&mut *conn)
    .await?;
    Ok(revoked)
}
