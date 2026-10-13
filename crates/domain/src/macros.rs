//! Macros that define text newtypes and status enums.

/// Defines a text newtype of 1 to `MAX_CHARS` characters, without control characters.
/// `Debug` shows the length only (ADR 0035).
macro_rules! text_type {
    ($(#[$doc:meta])* $name:ident, $max:expr) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            pub const MAX_CHARS: usize = $max;

            /// Removes the spaces at the ends, then checks the text.
            pub fn parse(input: &str) -> Result<Self, $crate::facts::TextError> {
                Ok(Self($crate::facts::checked_text(input, Self::MAX_CHARS)?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({} characters)", stringify!($name), self.0.chars().count())
            }
        }
    };
}

/// Defines a status enum with kebab-case `as_str` and `parse`, the list `ALL` of its statuses
/// and the list `NAMES` of their texts, both in the order of the variants.
/// With `: transitions`, it also defines `next_statuses` from the hand-written `change_to`.
macro_rules! status_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: [Self; [$($text),+].len()] = [$(Self::$variant),+];

            pub const NAMES: [&'static str; [$($text),+].len()] = [$($text),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            pub fn parse(input: &str) -> Option<Self> {
                match input {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
    ($(#[$doc:meta])* $name:ident: transitions { $($variant:ident => $text:literal),+ $(,)? }) => {
        status_enum!($(#[$doc])* $name { $($variant => $text),+ });

        impl $name {
            /// The statuses that a record in this status can change to.
            pub fn next_statuses(self) -> Vec<Self> {
                Self::ALL
                    .into_iter()
                    .filter(|next| self.change_to(*next).is_ok())
                    .collect()
            }
        }
    };
}
