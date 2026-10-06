//! Domain commands, queries and ports. The only way to change accepted state.

pub mod auth;
pub mod blobs;
pub mod caller;
pub mod clock;
pub mod events;
pub mod health;
pub mod jobs;
pub mod paging;
pub mod problem;
pub mod store;

pub use tada_domain as domain;
