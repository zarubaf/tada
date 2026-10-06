//! The German texts of the bot, from the shared Fluent files (ADR 0005).

use fluent_bundle::{FluentBundle, FluentResource};

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
        let Some(pattern) = self.0.get_message(id).and_then(|message| message.value()) else {
            return id.to_owned();
        };
        let mut errors = Vec::new();
        self.0
            .format_pattern(pattern, None, &mut errors)
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
        ] {
            assert_ne!(messages.get(id), id, "the message {id} is missing");
        }
    }
}
