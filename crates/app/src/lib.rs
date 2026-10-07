//! Domain commands, queries and ports. The only way to change accepted state.

pub mod audit;
pub mod auth;
pub mod blobs;
pub mod caller;
pub mod clock;
pub mod drafts;
pub mod events;
pub mod health;
pub mod identity;
pub mod jobs;
pub mod mail;
pub mod outbound;
pub mod paging;
pub mod problem;
pub mod public_url;
pub mod session;
pub mod store;
pub mod telegram;
pub mod uploads;

pub use tada_domain as domain;
