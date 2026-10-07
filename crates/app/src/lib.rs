//! Domain commands, queries and ports. The only way to change accepted state.

pub mod access;
pub mod audit;
pub mod auth;
pub mod blobs;
pub mod bootstrap;
pub mod caller;
pub mod clock;
pub mod drafts;
pub mod event_members;
pub mod events;
pub mod facts;
pub mod health;
pub mod identity;
pub mod jobs;
pub mod mail;
pub mod members;
pub mod outbound;
pub mod paging;
pub mod problem;
pub mod public_url;
pub mod rate_limit;
pub mod session;
pub mod sign_in;
pub mod sources;
pub mod store;
pub mod telegram;
pub mod uploads;

pub use tada_domain as domain;
