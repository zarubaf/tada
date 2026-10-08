//! The German texts of the bot, from the shared Fluent files (ADR 0005).

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use tada_app::problem::ProblemCode;

const DE_CH: &str = include_str!("../../../locales/de-CH/telegram.ftl");

pub(crate) struct Messages(FluentBundle<FluentResource>);

impl Messages {
    pub(crate) fn new() -> Self {
        let resource =
            FluentResource::try_new(DE_CH.to_owned()).unwrap_or_else(|(resource, _)| resource);
        let mut bundle = FluentBundle::new(vec!["de-CH".parse().unwrap_or_default()]);
        bundle.set_use_isolating(false);
        let _ = bundle.add_resource(resource);
        Self(bundle)
    }

    /// The text of the message `id`. A missing message shows its ID, so that the gap is visible.
    pub(crate) fn get(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// The text of the message `id` with one text argument.
    pub(crate) fn get_with(&self, id: &str, name: &str, value: &str) -> String {
        let mut args = FluentArgs::new();
        args.set(name, value.to_owned());
        self.format(id, Some(&args))
    }

    /// The message for a problem code (ADR 0037): `problem-<code>`.
    pub(crate) fn problem(&self, code: ProblemCode) -> String {
        self.get(&format!("problem-{}", code.as_str()))
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {
        let Some(pattern) = self.0.get_message(id).and_then(|message| message.value()) else {
            return id.to_owned();
        };
        let mut errors = Vec::new();
        self.0
            .format_pattern(pattern, args, &mut errors)
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_each_message_of_the_gateway() {
        let messages = Messages::new();
        for id in [
            "telegram-help",
            "telegram-link-claimed",
            "telegram-link-invalid",
            "telegram-error",
            "telegram-proposal-usage",
            "telegram-proposal-created",
            "telegram-proposal-reason",
            "telegram-not-linked",
            "telegram-ambiguous",
            "telegram-unknown-field",
            "telegram-value-invalid",
            "telegram-value-web-only",
        ] {
            assert_ne!(messages.get(id), id, "the message {id} is missing");
        }
    }

    #[test]
    fn has_a_message_for_each_problem_code() {
        let messages = Messages::new();
        for code in ProblemCode::ALL {
            let text = messages.problem(code);
            assert!(!text.starts_with("problem-"), "no message for {code:?}");
        }
    }

    #[test]
    fn names_the_event_in_the_confirmation() {
        let text =
            Messages::new().get_with("telegram-proposal-created", "event", "Open Day Testwil");
        assert!(text.contains("Open Day Testwil"), "{text}");
        assert!(!text.contains('{'), "{text}");
    }
}
