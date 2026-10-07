//! The outbound mail adapters (ADRs 0042 and 0057).
//!
//! `SmtpMailer` sends through an SMTP server with required TLS.
//! `MemoryMailer` records the messages for tests.
//! `FluentMailTexts` renders the German mail texts from `locales/de-CH/mail.ftl` (ADR 0005).

use std::time::Duration;

use async_trait::async_trait;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use secrecy::{ExposeSecret, SecretString};
use tada_app::domain::identity::Email;
use tada_app::mail::{MailTexts, Mailer, OutgoingMessage, Rendered, SendError};

/// How the connection to the SMTP server is secured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTls {
    /// TLS from the first byte.
    Implicit,
    /// A plain connection that upgrades with `STARTTLS`. The upgrade is required.
    StartTls,
    /// No TLS. Only for a local development server, so release builds have no such variant.
    #[cfg(any(debug_assertions, test))]
    None,
}

/// The connection settings of the SMTP adapter.
#[derive(Debug)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub tls: SmtpTls,
    pub credentials: Option<(String, SecretString)>,
    /// The sender address of all messages.
    pub from: Email,
    /// The time limit for each send. A send that exceeds it has an unknown outcome.
    pub timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum SmtpSetupError {
    #[error("the sender address is not valid")]
    Sender,
    #[error("cannot set up the SMTP transport: {0}")]
    Transport(#[from] lettre::transport::smtp::Error),
}

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    timeout: Duration,
}

impl std::fmt::Debug for SmtpMailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpMailer").finish_non_exhaustive()
    }
}

impl SmtpMailer {
    pub fn new(config: SmtpConfig) -> Result<Self, SmtpSetupError> {
        let builder = match config.tls {
            SmtpTls::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)?,
            SmtpTls::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)?
            }
            #[cfg(any(debug_assertions, test))]
            SmtpTls::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
        };
        let mut builder = builder.port(config.port).timeout(Some(config.timeout));
        if let Some((username, password)) = &config.credentials {
            builder = builder.credentials(Credentials::new(
                username.clone(),
                password.expose_secret().to_owned(),
            ));
        }
        let from = config
            .from
            .as_str()
            .parse()
            .map_err(|_| SmtpSetupError::Sender)?;
        Ok(Self {
            transport: builder.build(),
            from,
            timeout: config.timeout,
        })
    }

    fn build(&self, message: &OutgoingMessage) -> Result<Message, SendError> {
        let to: Mailbox = message
            .to
            .as_str()
            .parse()
            .map_err(|_| SendError::Rejected("the recipient address is not valid".into()))?;
        Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(message.subject.clone())
            .message_id(Some(message.message_id.clone()))
            .multipart(MultiPart::alternative_plain_html(
                message.text.clone(),
                message.html.clone(),
            ))
            .map_err(|_| SendError::Rejected("the message cannot be built".into()))
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), SendError> {
        let message = self.build(message)?;
        // The transport has its own time limit. This outer limit also covers a stuck connection.
        let sent = tokio::time::timeout(self.timeout * 2, self.transport.send(message)).await;
        match sent {
            Err(_) => Err(SendError::Unknown),
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(classify(&error)),
        }
    }
}

/// Maps an SMTP error to a send error.
/// The reason has the status code only, because the server text can repeat the address.
///
/// Only an answer of the server or a failed TLS handshake proves that the server did not take
/// the message. Any other error can also happen after DATA, so its outcome is unknown, and the
/// worker must not retry it blindly (ADR 0042).
fn classify(error: &lettre::transport::smtp::Error) -> SendError {
    if let Some(code) = error.status() {
        let reason = format!("SMTP status {code}");
        return if error.is_permanent() {
            SendError::Rejected(reason)
        } else {
            SendError::Temporary(reason)
        };
    }
    if error.is_tls() {
        return SendError::Temporary("the TLS connection failed".to_owned());
    }
    SendError::Unknown
}

/// Records the messages instead of sending them.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Default)]
pub struct MemoryMailer {
    sent: std::sync::Mutex<Vec<OutgoingMessage>>,
}

#[cfg(any(test, feature = "testing"))]
impl MemoryMailer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The messages in the order of the sends.
    pub fn sent(&self) -> Vec<OutgoingMessage> {
        self.sent
            .lock()
            .map(|sent| sent.clone())
            .unwrap_or_default()
    }
}

#[cfg(any(test, feature = "testing"))]
#[async_trait]
impl Mailer for MemoryMailer {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), SendError> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push(message.clone());
        }
        Ok(())
    }
}

const DE_CH: &str = include_str!("../../../locales/de-CH/mail.ftl");

/// The mail texts from the Fluent file.
/// German (Switzerland) is the only locale, so every locale gets it.
pub struct FluentMailTexts {
    bundle: FluentBundle<FluentResource>,
}

impl std::fmt::Debug for FluentMailTexts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FluentMailTexts").finish_non_exhaustive()
    }
}

/// The messages that the mail texts need.
const REQUIRED_MESSAGES: [&str; 4] = [
    "magic-link-subject",
    "magic-link-body",
    "invitation-subject",
    "invitation-body",
];

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum MailTextsError {
    #[error("the mail text file does not parse")]
    Parse,
    #[error("the mail text file has no message {0}")]
    Missing(&'static str),
}

impl FluentMailTexts {
    /// Loads the German texts. It fails at startup, not at the first send.
    pub fn new() -> Result<Self, MailTextsError> {
        Self::from_source(DE_CH)
    }

    fn from_source(source: &str) -> Result<Self, MailTextsError> {
        let resource =
            FluentResource::try_new(source.to_owned()).map_err(|_| MailTextsError::Parse)?;
        let mut bundle = FluentBundle::new_concurrent(vec!["de-CH".parse().unwrap_or_default()]);
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .map_err(|_| MailTextsError::Parse)?;
        for id in REQUIRED_MESSAGES {
            if bundle.get_message(id).and_then(|m| m.value()).is_none() {
                return Err(MailTextsError::Missing(id));
            }
        }
        Ok(Self { bundle })
    }

    /// The text of the message `id`. `from_source` checked that the required messages exist.
    fn get(&self, id: &str, args: &FluentArgs<'_>) -> String {
        let Some(pattern) = self
            .bundle
            .get_message(id)
            .and_then(|message| message.value())
        else {
            return id.to_owned();
        };
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, Some(args), &mut errors)
            .into_owned()
    }

    fn render(&self, id: &str, args: &FluentArgs<'_>, link: &str) -> Rendered {
        let text = self.get(&format!("{id}-body"), args);
        Rendered {
            subject: self.get(&format!("{id}-subject"), args),
            html: html_from_text(&text, link),
            text,
        }
    }
}

impl MailTexts for FluentMailTexts {
    fn magic_link(&self, _locale: &str, link: &str) -> Rendered {
        let mut args = FluentArgs::new();
        args.set("link", FluentValue::from(link));
        self.render("magic-link", &args, link)
    }

    fn invitation(&self, _locale: &str, organization_name: &str, link: &str) -> Rendered {
        let mut args = FluentArgs::new();
        args.set("organization", FluentValue::from(organization_name));
        args.set("link", FluentValue::from(link));
        self.render("invitation", &args, link)
    }
}

/// A simple HTML part from the text part: one paragraph for each block, and the link as an anchor.
/// Every inserted value is escaped. The HTML has no remote content and no images.
fn html_from_text(text: &str, link: &str) -> String {
    let escaped_link = escape_html(link);
    let anchor = format!("<a href=\"{escaped_link}\">{escaped_link}</a>");
    let paragraphs: Vec<String> = text
        .split("\n\n")
        .map(|paragraph| {
            let escaped = escape_html(paragraph.trim()).replace(&escaped_link, &anchor);
            format!("<p>{}</p>", escaped.replace('\n', "<br>\n"))
        })
        .collect();
    format!(
        "<!doctype html>\n<html lang=\"de-CH\"><body>\n{}\n</body></html>\n",
        paragraphs.join("\n")
    )
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}
#[cfg(test)]
mod tests {
    use std::io::{BufRead, Read, Write};
    use std::net::{TcpListener, TcpStream};

    use testcontainers_modules::testcontainers::core::{IntoContainerPort, WaitFor};
    use testcontainers_modules::testcontainers::runners::AsyncRunner;
    use testcontainers_modules::testcontainers::{ContainerAsync, GenericImage};

    use super::*;

    /// The Mailpit image of `compose.yaml`.
    const MAILPIT: (&str, &str) = ("axllent/mailpit", "v1.31.4");

    const LINK: &str = "https://tada.example.org/#token=abc&x=1";

    #[test]
    fn renders_each_text_with_the_link() {
        let texts = FluentMailTexts::new().unwrap();
        let magic = texts.magic_link("de-CH", LINK);
        let invitation = texts.invitation("de-CH", "Fliegerclub Musterhausen", LINK);
        for rendered in [&magic, &invitation] {
            assert!(!rendered.subject.is_empty());
            assert!(rendered.text.contains(LINK), "{}", rendered.text);
            assert!(
                rendered.html.contains("&amp;x=1"),
                "the HTML escapes the link: {}",
                rendered.html
            );
            assert!(rendered.html.contains("<a href=\""));
        }
        assert!(magic.text.contains("15 Minuten"));
        assert!(magic.text.contains("ignorieren"));
        assert!(invitation.text.contains("7 Tage"));
        assert!(invitation.subject.contains("Fliegerclub Musterhausen"));
        assert!(invitation.text.contains("Fliegerclub Musterhausen"));
    }

    #[test]
    fn escapes_the_organization_name_in_the_html() {
        let rendered = FluentMailTexts::new().unwrap().invitation(
            "de-CH",
            "<script>alert(1)</script> & Co",
            LINK,
        );
        assert!(!rendered.html.contains("<script>"), "{}", rendered.html);
        assert!(rendered.html.contains("&lt;script&gt;"));
        assert!(rendered.html.contains("&amp; Co"));
    }

    #[test]
    fn html_has_no_remote_content() {
        let rendered = FluentMailTexts::new().unwrap().magic_link("de-CH", LINK);
        assert!(!rendered.html.contains("<img"));
        assert!(!rendered.html.contains("src="));
    }

    fn message(id: &str) -> OutgoingMessage {
        OutgoingMessage {
            to: Email::parse("anna@example.org").unwrap(),
            subject: "Anmeldung".into(),
            text: "Der Link steht im Text.".into(),
            html: "<p>Der Link steht im HTML.</p>".into(),
            message_id: id.into(),
        }
    }

    fn config(port: u16, timeout: Duration) -> SmtpConfig {
        SmtpConfig {
            host: "127.0.0.1".into(),
            port,
            tls: SmtpTls::None,
            credentials: None,
            from: Email::parse("tada@example.org").unwrap(),
            timeout,
        }
    }

    #[tokio::test]
    async fn the_memory_mailer_records_the_messages_in_order() {
        let mailer = MemoryMailer::new();
        mailer.send(&message("<1@example.org>")).await.unwrap();
        mailer.send(&message("<2@example.org>")).await.unwrap();
        let ids: Vec<String> = mailer.sent().into_iter().map(|m| m.message_id).collect();
        assert_eq!(ids, ["<1@example.org>", "<2@example.org>"]);
    }

    /// A GET request to the Mailpit API. HTTP/1.0 gives a body without chunks.
    fn get(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response.split_once("\r\n\r\n").unwrap().1.to_owned()
    }

    struct TestMailpit {
        smtp_port: u16,
        http_port: u16,
        _container: ContainerAsync<GenericImage>,
    }

    impl TestMailpit {
        async fn start() -> Self {
            let container = GenericImage::new(MAILPIT.0, MAILPIT.1)
                .with_wait_for(WaitFor::message_on_stdout("[http] starting on"))
                .with_exposed_port(1025.tcp())
                .with_exposed_port(8025.tcp())
                .start()
                .await
                .expect("cannot start Mailpit; is Docker running?");
            Self {
                smtp_port: container.get_host_port_ipv4(1025.tcp()).await.unwrap(),
                http_port: container.get_host_port_ipv4(8025.tcp()).await.unwrap(),
                _container: container,
            }
        }

        /// The raw text of the first message, after it arrived.
        async fn first_raw_message(&self) -> String {
            for _ in 0..50 {
                let list: serde_json::Value =
                    serde_json::from_str(&get(self.http_port, "/api/v1/messages")).unwrap();
                if let Some(id) = list["messages"][0]["ID"].as_str() {
                    return get(self.http_port, &format!("/api/v1/message/{id}/raw"));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            panic!("no message arrived at Mailpit");
        }
    }

    #[tokio::test]
    async fn sends_both_parts_and_the_message_id_through_smtp() {
        let mailpit = TestMailpit::start().await;
        let mailer = SmtpMailer::new(config(mailpit.smtp_port, Duration::from_secs(10))).unwrap();

        mailer
            .send(&message("<abc123@tada.example.org>"))
            .await
            .unwrap();

        let raw = mailpit.first_raw_message().await.to_lowercase();
        assert!(
            raw.contains("message-id: <abc123@tada.example.org>"),
            "{raw}"
        );
        assert!(raw.contains("multipart/alternative"), "{raw}");
        assert!(raw.contains("content-type: text/plain"), "{raw}");
        assert!(raw.contains("content-type: text/html"), "{raw}");
        assert!(raw.contains("der link steht im text."), "{raw}");
        assert!(raw.contains("der link steht im html."), "{raw}");
    }

    /// An SMTP server that sends `greeting` and then waits.
    fn fake_server(greeting: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(greeting.as_bytes());
                std::thread::sleep(Duration::from_secs(3));
            }
        });
        port
    }

    #[tokio::test]
    async fn a_permanent_refusal_is_rejected_without_the_server_text() {
        let port = fake_server("554 anna@example.org is banned\r\n");
        let mailer = SmtpMailer::new(config(port, Duration::from_secs(2))).unwrap();
        let error = mailer.send(&message("<1@example.org>")).await.unwrap_err();
        assert!(matches!(&error, SendError::Rejected(_)), "{error:?}");
        assert!(!format!("{error:?}").contains("anna"), "{error:?}");
    }

    #[tokio::test]
    async fn a_temporary_refusal_is_temporary() {
        let port = fake_server("421 try again later\r\n");
        let mailer = SmtpMailer::new(config(port, Duration::from_secs(2))).unwrap();
        let error = mailer.send(&message("<1@example.org>")).await.unwrap_err();
        assert!(matches!(&error, SendError::Temporary(_)), "{error:?}");
    }

    #[tokio::test]
    async fn a_silent_server_gives_an_unknown_outcome() {
        let port = fake_server("");
        let mailer = SmtpMailer::new(config(port, Duration::from_millis(300))).unwrap();
        let error = mailer.send(&message("<1@example.org>")).await.unwrap_err();
        assert_eq!(error, SendError::Unknown);
    }

    /// An SMTP server that accepts the whole message and then drops the connection.
    fn dropping_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let _ = stream.write_all(b"220 hello\r\n");
            let mut line = String::new();
            let mut in_data = false;
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let reply: &[u8] = if in_data {
                    if line == ".\r\n" {
                        return; // Drop the connection before the answer to DATA.
                    }
                    b""
                } else if line.starts_with("DATA") {
                    in_data = true;
                    b"354 go on\r\n"
                } else {
                    b"250 ok\r\n"
                };
                let _ = stream.write_all(reply);
                line.clear();
            }
        });
        port
    }

    #[tokio::test]
    async fn a_connection_that_drops_after_data_gives_an_unknown_outcome() {
        let mailer = SmtpMailer::new(config(dropping_server(), Duration::from_secs(5))).unwrap();
        let error = mailer.send(&message("<1@example.org>")).await.unwrap_err();
        assert_eq!(error, SendError::Unknown);
    }

    #[test]
    fn rejects_a_text_file_that_does_not_parse() {
        let error = FluentMailTexts::from_source("magic-link-subject = { \n").unwrap_err();
        assert!(matches!(error, MailTextsError::Parse), "{error:?}");
    }

    #[test]
    fn rejects_a_text_file_without_a_required_message() {
        let error = FluentMailTexts::from_source("magic-link-subject = Hallo\n").unwrap_err();
        assert!(matches!(error, MailTextsError::Missing(_)), "{error:?}");
    }
}
