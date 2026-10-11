//! The texts of the AI PM digest, from the shared Fluent file (ADRs 0005 and 0076).

use std::fmt;

use fluent_bundle::{FluentArgs, FluentValue};
use tada_app::domain::reminders::{Answer, RecordKind};
use tada_app::pm::send::{ButtonLabel, DigestBody, DigestMessage, DigestTexts, TemplateItem};

use crate::messages::Messages;

/// The German texts of the digest. Each locale gets them until a second language exists.
pub struct FluentDigestTexts(Messages);

impl fmt::Debug for FluentDigestTexts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FluentDigestTexts").finish_non_exhaustive()
    }
}

impl Default for FluentDigestTexts {
    fn default() -> Self {
        Self(Messages::new())
    }
}

impl FluentDigestTexts {
    pub fn new() -> Self {
        Self::default()
    }

    fn item(&self, item: &TemplateItem) -> String {
        let mut args = FluentArgs::new();
        args.set("number", FluentValue::from(item.number));
        args.set(
            "kind",
            match item.kind {
                RecordKind::Action => "action",
                RecordKind::Commitment => "commitment",
            },
        );
        args.set("title", item.title.as_str());
        args.set("event", item.event_name.as_str());
        args.set("reason", item.reason.as_str());
        args.set(
            "due",
            item.due_date.map_or_else(
                || "none".to_owned(),
                |date| date.strftime("%d.%m.%Y").to_string(),
            ),
        );
        self.0.get_args("telegram-digest-item", &args)
    }
}

impl DigestTexts for FluentDigestTexts {
    fn render(&self, _locale: &str, message: &DigestMessage) -> String {
        let mut paragraphs = Vec::new();
        match &message.body {
            DigestBody::Template(items) => {
                paragraphs.push(self.0.get("telegram-digest-intro"));
                paragraphs.extend(items.iter().map(|item| self.item(item)));
            }
            DigestBody::Worded { intro, lines } => {
                paragraphs.push(intro.clone());
                // The number of the item, not the position of the line, so that the line names its buttons.
                paragraphs.extend(lines.iter().map(|line| {
                    let mut args = FluentArgs::new();
                    args.set("number", FluentValue::from(line.number));
                    args.set("text", line.text.as_str());
                    self.0.get_args("telegram-digest-worded-item", &args)
                }));
            }
        }
        if message.more > 0 {
            let mut args = FluentArgs::new();
            args.set("count", FluentValue::from(message.more));
            paragraphs.push(self.0.get_args("telegram-digest-more", &args));
        }
        paragraphs.join("\n\n")
    }

    fn button(&self, _locale: &str, label: ButtonLabel) -> String {
        let (id, number) = match label {
            ButtonLabel::Answer(number, answer) => (
                match answer {
                    Answer::OnTrack => "telegram-digest-on-track",
                    Answer::Blocked => "telegram-digest-blocked",
                    Answer::Done => "telegram-digest-done",
                    Answer::Fulfilled => "telegram-digest-fulfilled",
                },
                number,
            ),
            ButtonLabel::Open(number) => ("telegram-digest-open", number),
            ButtonLabel::MyWork => return self.0.get("telegram-digest-my-work"),
        };
        let mut args = FluentArgs::new();
        args.set("number", FluentValue::from(number));
        self.0.get_args(id, &args)
    }
}

#[cfg(test)]
mod tests {
    use tada_app::domain::reminders::Reason;
    use tada_app::pm::send::{TemplateItem, WordedItem};

    use super::*;

    fn item(number: usize, kind: RecordKind, reason: Reason, due: Option<&str>) -> TemplateItem {
        TemplateItem {
            number,
            kind,
            title: "Festzelt aufstellen".to_owned(),
            event_name: "Flugtag 2028".to_owned(),
            due_date: due.map(|due| due.parse().unwrap()),
            reason,
        }
    }

    #[test]
    fn the_template_names_each_item_with_its_reason() {
        let texts = FluentDigestTexts::new();
        let message = DigestMessage {
            body: DigestBody::Template(vec![
                item(1, RecordKind::Action, Reason::Overdue, Some("2028-05-30")),
                item(
                    2,
                    RecordKind::Commitment,
                    Reason::DueSoon,
                    Some("2028-06-01"),
                ),
                item(3, RecordKind::Action, Reason::Blocked, None),
                item(4, RecordKind::Action, Reason::FollowUp, None),
            ]),
            more: 1,
        };
        let text = texts.render("de-CH", &message);
        assert_eq!(
            text,
            "Guten Tag. Diese Einträge brauchen Ihre Aufmerksamkeit:\n\n\
             1. Aufgabe: Festzelt aufstellen (Flugtag 2028), überfällig, fällig am 30.05.2028\n\n\
             2. Zusage: Festzelt aufstellen (Flugtag 2028), fällig am 01.06.2028\n\n\
             3. Aufgabe: Festzelt aufstellen (Flugtag 2028), blockiert\n\n\
             4. Aufgabe: Festzelt aufstellen (Flugtag 2028), noch keine Antwort\n\n\
             … und 1 weiterer Eintrag in „Meine Arbeit“."
        );
    }

    #[test]
    fn the_wording_keeps_the_numbers_of_the_buttons() {
        let texts = FluentDigestTexts::new();
        let message = DigestMessage {
            body: DigestBody::Worded {
                intro: "Zwei Punkte für heute.".to_owned(),
                lines: vec![
                    WordedItem {
                        number: 1,
                        text: "Das Festzelt.".to_owned(),
                    },
                    WordedItem {
                        number: 2,
                        text: "Die Bänke.".to_owned(),
                    },
                ],
            },
            more: 3,
        };
        assert_eq!(
            texts.render("de-CH", &message),
            "Zwei Punkte für heute.\n\n1. Das Festzelt.\n\n2. Die Bänke.\n\n\
             … und 3 weitere Einträge in „Meine Arbeit“."
        );
        assert_eq!(
            texts.button("de-CH", ButtonLabel::Answer(2, Answer::Fulfilled)),
            "2 Erfüllt"
        );
        assert_eq!(texts.button("de-CH", ButtonLabel::Open(10)), "10 Öffnen");
        assert_eq!(
            texts.button("de-CH", ButtonLabel::MyWork),
            "Meine Arbeit öffnen"
        );
    }
}
