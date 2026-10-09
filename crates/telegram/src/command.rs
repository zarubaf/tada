//! The words of the command `/vorschlag <EVENTKEY> <field_key> <value…>`, and the commands of the
//! link without arguments.

use std::ops::Range;

/// The name of the command. In a chat with several bots, Telegram can add `@botname`.
const PROPOSE: &str = "/vorschlag";

/// The command that accepts the newest claim of a link code (ADR 0011).
const ACCEPT: &str = "/bestaetigen";
/// The command that ends the link of the account and rejects its open claims.
const UNLINK: &str = "/trennen";

/// A command of the link, without arguments.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LinkCommand {
    Accept,
    Unlink,
}

/// Returns `None` if `text` is not `/bestaetigen` or `/trennen`. Words after the command do not count.
pub(crate) fn parse_link_command(text: &str) -> Option<LinkCommand> {
    let (command, _) = word(text, 0)?;
    let name = command.split_once('@').map_or(command, |(name, _)| name);
    if name.eq_ignore_ascii_case(ACCEPT) {
        Some(LinkCommand::Accept)
    } else if name.eq_ignore_ascii_case(UNLINK) {
        Some(LinkCommand::Unlink)
    } else {
        None
    }
}

/// A parsed `/vorschlag` command. It borrows the message text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProposeCommand<'a> {
    pub event_key: &'a str,
    pub field_key: &'a str,
    /// The value part, in characters of the message text.
    pub value: Range<usize>,
}

/// The command `/vorschlag` without its event key, field key or value.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Incomplete;

/// Returns `None` if `text` is not the command `/vorschlag`.
pub(crate) fn parse_propose(text: &str) -> Option<Result<ProposeCommand<'_>, Incomplete>> {
    let (command, rest) = word(text, 0)?;
    let name = command.split_once('@').map_or(command, |(name, _)| name);
    if !name.eq_ignore_ascii_case(PROPOSE) {
        return None;
    }
    Some(arguments(text, rest))
}

fn arguments(text: &str, from: usize) -> Result<ProposeCommand<'_>, Incomplete> {
    let (event_key, from) = word(text, from).ok_or(Incomplete)?;
    let (field_key, from) = word(text, from).ok_or(Incomplete)?;
    let start = from + text[from..].len() - text[from..].trim_start().len();
    let end = text.trim_end().len();
    if start >= end {
        return Err(Incomplete);
    }
    Ok(ProposeCommand {
        event_key,
        field_key,
        value: characters(text, start)..characters(text, end),
    })
}

/// The next word at or after the byte `from`, and the byte after it.
fn word(text: &str, from: usize) -> Option<(&str, usize)> {
    let rest = &text[from..];
    let start = from + rest.len() - rest.trim_start().len();
    let length = text[start..]
        .find(char::is_whitespace)
        .unwrap_or(text.len() - start);
    (length > 0).then(|| (&text[start..start + length], start + length))
}

/// The number of characters before the byte `offset`.
fn characters(text: &str, offset: usize) -> usize {
    text[..offset].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_the_command_into_its_words_and_the_value_range() {
        let text = "/vorschlag TEST30 date_window 2030-06..2030-07";
        let command = parse_propose(text).unwrap().unwrap();
        assert_eq!(command.event_key, "TEST30");
        assert_eq!(command.field_key, "date_window");
        assert_eq!(&text[command.value.clone()], "2030-06..2030-07");
    }

    #[test]
    fn keeps_spaces_inside_the_value_and_counts_characters() {
        let text = "/vorschlag  TEST30\tvenue   Flugplatz Zürich Süd \n";
        let command = parse_propose(text).unwrap().unwrap();
        let value: String = text
            .chars()
            .skip(command.value.start)
            .take(command.value.len())
            .collect();
        assert_eq!(value, "Flugplatz Zürich Süd");
    }

    #[test]
    fn accepts_the_name_of_the_bot_and_any_case() {
        for text in ["/vorschlag@tada_bot A1 f x", "/Vorschlag A1 f x"] {
            assert!(parse_propose(text).unwrap().is_ok(), "{text}");
        }
    }

    #[test]
    fn finds_the_commands_of_the_link() {
        assert_eq!(
            parse_link_command("/bestaetigen"),
            Some(LinkCommand::Accept)
        );
        assert_eq!(
            parse_link_command("/Trennen@tada_bot"),
            Some(LinkCommand::Unlink)
        );
        for text in ["/start code", "bestaetigen", "", "/vorschlag A1 f x"] {
            assert_eq!(parse_link_command(text), None, "{text:?}");
        }
    }

    #[test]
    fn rejects_other_text_and_incomplete_commands() {
        assert_eq!(parse_propose("hello A1 f x"), None);
        assert_eq!(parse_propose("/start code"), None);
        assert_eq!(parse_propose(""), None);
        for text in [
            "/vorschlag",
            "/vorschlag A1",
            "/vorschlag A1 f",
            "/vorschlag A1 f  ",
        ] {
            assert_eq!(parse_propose(text), Some(Err(Incomplete)), "{text:?}");
        }
    }
}
