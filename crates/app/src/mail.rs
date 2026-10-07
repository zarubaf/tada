//! The outbound mail port (ADRs 0042 and 0057).
//!
//! The port lives here, and the adapters implement it.
//! Configuration selects the adapter.

use std::fmt;

use async_trait::async_trait;
use tada_domain::identity::Email;

/// One message to one recipient.
/// `Debug` hides the address and the bodies (ADR 0035).
#[derive(Clone, PartialEq, Eq)]
pub struct OutgoingMessage {
    pub to: Email,
    pub subject: String,
    pub text: String,
    pub html: String,
    /// The `Message-ID` header, with angle brackets.
    pub message_id: String,
}

impl fmt::Debug for OutgoingMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutgoingMessage")
            .field("message_id", &self.message_id)
            .finish_non_exhaustive()
    }
}

/// Why a send did not succeed.
/// The reason texts never contain an address.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// The server refused the message for good. A retry does not help.
    #[error("the mail server rejected the message: {0}")]
    Rejected(String),
    /// The server could not take the message now. A retry can work.
    #[error("the mail server could not take the message now: {0}")]
    Temporary(String),
    /// The send timed out. The server can have taken the message.
    #[error("the outcome of the send is unknown")]
    Unknown,
}

#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), SendError>;
}

/// The subject and the two bodies of one mail text.
/// The bodies hold the link with its token, so `Debug` hides them (ADRs 0008 and 0035).
#[derive(Clone, PartialEq, Eq)]
pub struct Rendered {
    pub subject: String,
    pub text: String,
    pub html: String,
}

impl fmt::Debug for Rendered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rendered").finish_non_exhaustive()
    }
}

/// The texts of the mails that tada sends.
/// The implementation owns the translation, so that `app` stays free of it (ADR 0005).
pub trait MailTexts: Send + Sync {
    fn magic_link(&self, locale: &str, link: &str) -> Rendered;
    fn invitation(&self, locale: &str, organization_name: &str, link: &str) -> Rendered;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tada_domain::identity::Email;

    #[test]
    fn debug_of_a_message_hides_the_address_and_the_bodies() {
        let message = OutgoingMessage {
            to: Email::parse("anna@example.org").unwrap(),
            subject: "Anmeldung".into(),
            text: "geheimer-link-text".into(),
            html: "<p>geheimer-link-html</p>".into(),
            message_id: "<id@tada.example.org>".into(),
        };
        let shown = format!("{message:?}");
        assert!(!shown.contains("anna"), "{shown}");
        assert!(!shown.contains("geheimer"), "{shown}");
        assert!(shown.contains("<id@tada.example.org>"), "{shown}");
    }

    #[test]
    fn debug_of_a_rendered_text_hides_the_link() {
        let rendered = Rendered {
            subject: "Anmeldung".into(),
            text: "https://tada.example.org/#token=geheim".into(),
            html: "<a>geheim</a>".into(),
        };
        assert!(!format!("{rendered:?}").contains("geheim"));
    }
}
