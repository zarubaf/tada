//! Persons, institutions and the parties that make a commitment (ADR 0038).

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::facts::{TextError, checked_text};
use crate::ids::{InstitutionId, PersonId};

/// The name of a person or an institution: 1 to 200 characters, without control characters and without spaces at the ends.
#[derive(Clone, PartialEq, Eq)]
pub struct PartyName(String);

impl PartyName {
    pub const MAX_CHARS: usize = 200;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A name is personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for PartyName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PartyName({} characters)", self.0.chars().count())
    }
}

/// A phone number as written: 1 to 50 characters, without control characters and without spaces at the ends.
#[derive(Clone, PartialEq, Eq)]
pub struct PhoneNumber(String);

impl PhoneNumber {
    pub const MAX_CHARS: usize = 50;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A phone number is personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for PhoneNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PhoneNumber({} characters)", self.0.chars().count())
    }
}

/// The kind of an institution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstitutionKind {
    Authority,
    Company,
    Club,
    Other,
}

impl InstitutionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "authority",
            Self::Company => "company",
            Self::Club => "club",
            Self::Other => "other",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "authority" => Some(Self::Authority),
            "company" => Some(Self::Company),
            "club" => Some(Self::Club),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// The party that makes a commitment: exactly one person or one institution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    Person(PersonId),
    Institution(InstitutionId),
}

impl Party {
    pub fn as_uuid(self) -> uuid::Uuid {
        match self {
            Self::Person(id) => id.as_uuid(),
            Self::Institution(id) => id.as_uuid(),
        }
    }
}

/// The form of a name for matching (ADR 0069): Unicode NFKC, lowercase, without diacritics, `ß` as `ss`,
/// each character that is not a letter or a digit as a space, and single spaces.
/// For example, "Müller-Bau AG" and "Müller Bau AG" both give "muller bau ag".
pub fn normalized_name(input: &str) -> String {
    let mut folded = String::with_capacity(input.len());
    for c in input
        .nfkc()
        .flat_map(char::to_lowercase)
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
    {
        match c {
            'ß' => folded.push_str("ss"),
            c if c.is_alphanumeric() => folded.push(c),
            _ => folded.push(' '),
        }
    }
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_name_ignores_case_accents_and_spaces() {
        assert_eq!(normalized_name("  Müller  AG"), "muller ag");
        assert_eq!(normalized_name("muller ag"), "muller ag");
        assert_eq!(normalized_name("Mu\u{308}ller\tAG"), "muller ag");
        assert_eq!(normalized_name("ＡＢＣ"), "abc");
    }

    #[test]
    fn normalized_name_treats_punctuation_as_spaces() {
        assert_eq!(
            normalized_name("Müller-Bau AG"),
            normalized_name("Müller Bau AG")
        );
        assert_eq!(normalized_name("Müller-Bau AG"), "muller bau ag");
        assert_eq!(
            normalized_name("St. Gallen, Ost/West"),
            "st gallen ost west"
        );
        assert_eq!(normalized_name("Meier & Söhne"), "meier sohne");
        assert_eq!(normalized_name("D'Angelo"), "d angelo");
        assert_eq!(normalized_name("Muster AG (Bern)"), "muster ag bern");
    }

    #[test]
    fn normalized_name_folds_sharp_s() {
        assert_eq!(normalized_name("Strasse"), normalized_name("Straße"));
        assert_eq!(normalized_name("STRAẞE"), "strasse");
    }

    #[test]
    fn text_types_refuse_text_over_the_limit() {
        assert!(PartyName::parse(&"a".repeat(200)).is_ok());
        assert_eq!(PartyName::parse(&"a".repeat(201)), Err(TextError::TooLong));
        assert!(PhoneNumber::parse(&"1".repeat(50)).is_ok());
        assert_eq!(PhoneNumber::parse(&"1".repeat(51)), Err(TextError::TooLong));
        assert_eq!(PartyName::parse("  "), Err(TextError::Empty));
    }

    #[test]
    fn institution_kinds_round_trip() {
        for kind in [
            InstitutionKind::Authority,
            InstitutionKind::Company,
            InstitutionKind::Club,
            InstitutionKind::Other,
        ] {
            assert_eq!(InstitutionKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(InstitutionKind::parse("bank"), None);
    }
}
