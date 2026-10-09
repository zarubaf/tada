//! The role DTOs that more than one resource uses.

use serde::{Deserialize, Serialize};
use tada_app::caller::OrganizationRole as Role;
use utoipa::ToSchema;

/// The role of a member in an organization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OrganizationRole {
    Owner,
    Admin,
    Member,
}

impl From<Role> for OrganizationRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Admin => Self::Admin,
            Role::Member => Self::Member,
        }
    }
}

impl From<OrganizationRole> for Role {
    fn from(role: OrganizationRole) -> Self {
        match role {
            OrganizationRole::Owner => Self::Owner,
            OrganizationRole::Admin => Self::Admin,
            OrganizationRole::Member => Self::Member,
        }
    }
}
