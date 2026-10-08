//! Value types and fact values (ADR 0049, layer 2).
//!
//! A `ValueType` belongs to a field definition. A `FactValue` is one value of a fact.
//! The constructors of the parts check them, and `FactValue::check` compares a value with a value type.

use std::cmp::Ordering;
use std::collections::HashSet;

use jiff::civil;

use super::{KeyError, TextError, checked_text, snake_case_key};
use crate::ids::{DocumentId, EventId};

/// The kind of value that a field holds, with its unit, currency or list of choices.
/// The value type of a field never changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Text,
    Boolean,
    Quantity {
        unit: Unit,
    },
    Money {
        currency: Currency,
    },
    Date,
    /// `granularity` is `None` if the field accepts each granularity.
    DateWindow {
        granularity: Option<Granularity>,
    },
    /// Only `choice` fields can hold several values.
    Choice {
        values: Vec<ChoiceValue>,
        multiple: bool,
    },
    Reference {
        target: ReferenceTarget,
    },
}

/// One value of a fact. Use `check` to compare it with the value type of its field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactValue {
    Text(ShortText),
    Boolean(bool),
    /// A number or a range, in the unit of the field.
    Quantity(Range<Decimal>),
    /// An amount or a range, in the currency of the field.
    Money(Range<MinorUnits>),
    Date(civil::Date),
    DateWindow(DateWindow),
    /// One key, or several keys if the field allows them. The keys are unique.
    Choice(Vec<ChoiceKey>),
    Reference(ReferenceId),
}

/// A fact value with the mark "approximate", for example "about 20,000".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valued {
    pub value: FactValue,
    pub approximate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    #[error("the value does not have the value type of the field")]
    TypeMismatch,
    #[error("a choice value has at least one key")]
    NoChoice,
    #[error("a choice value has each key only once")]
    DuplicateChoice,
    #[error("the field allows only one choice")]
    SeveralChoices,
    #[error("the key is not in the list of choices of the field")]
    UnknownChoice,
    #[error("the date window does not have the granularity of the field")]
    GranularityMismatch,
    #[error("the reference does not point to the kind of record of the field")]
    ReferenceTargetMismatch,
    #[error("a decimal has a scale of at most {}", Decimal::MAX_SCALE)]
    ScaleTooLarge,
    #[error("the start of a range is not after its end")]
    RangeOrder,
    #[error("the start of a date window is not after its end")]
    DateWindowOrder,
    #[error("a currency is an ISO 4217 code of three capital letters")]
    Currency,
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error(transparent)]
    Text(#[from] TextError),
}

impl FactValue {
    /// Returns `Ok` if the value matches `value_type`.
    pub fn check(&self, value_type: &ValueType) -> Result<(), ValueError> {
        match (self, value_type) {
            (Self::Text(_), ValueType::Text)
            | (Self::Boolean(_), ValueType::Boolean)
            | (Self::Quantity(_), ValueType::Quantity { .. })
            | (Self::Money(_), ValueType::Money { .. })
            | (Self::Date(_), ValueType::Date) => Ok(()),
            (Self::DateWindow(window), ValueType::DateWindow { granularity }) => {
                match granularity {
                    Some(granularity) if *granularity != window.granularity() => {
                        Err(ValueError::GranularityMismatch)
                    }
                    _ => Ok(()),
                }
            }
            (Self::Choice(keys), ValueType::Choice { values, multiple }) => {
                check_choice(keys, values, *multiple)
            }
            (Self::Reference(reference), ValueType::Reference { target }) => {
                if reference.target() == *target {
                    Ok(())
                } else {
                    Err(ValueError::ReferenceTargetMismatch)
                }
            }
            _ => Err(ValueError::TypeMismatch),
        }
    }
}

fn check_choice(
    keys: &[ChoiceKey],
    values: &[ChoiceValue],
    multiple: bool,
) -> Result<(), ValueError> {
    if keys.is_empty() {
        return Err(ValueError::NoChoice);
    }
    let mut seen = HashSet::new();
    if !keys.iter().all(|key| seen.insert(key)) {
        return Err(ValueError::DuplicateChoice);
    }
    if !multiple && keys.len() > 1 {
        return Err(ValueError::SeveralChoices);
    }
    if keys
        .iter()
        .all(|key| values.iter().any(|value| value.key == *key))
    {
        Ok(())
    } else {
        Err(ValueError::UnknownChoice)
    }
}

/// A short text: 1 to 200 characters, without control characters and without spaces at the ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortText(String);

impl ShortText {
    pub const MAX_CHARS: usize = 200;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, ValueError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A decimal number: `units` × 10^-`scale`. It has no float, so it compares exactly.
/// The constructor removes trailing zeros, so `1.50` and `1.5` are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal {
    units: i64,
    scale: u8,
}

impl Decimal {
    pub const MAX_SCALE: u8 = 6;

    pub fn new(units: i64, scale: u8) -> Result<Self, ValueError> {
        if scale > Self::MAX_SCALE {
            return Err(ValueError::ScaleTooLarge);
        }
        let (mut units, mut scale) = (units, scale);
        while scale > 0 && units % 10 == 0 {
            units /= 10;
            scale -= 1;
        }
        Ok(Self { units, scale })
    }

    /// An integer.
    pub const fn integer(units: i64) -> Self {
        Self { units, scale: 0 }
    }

    pub const fn units(self) -> i64 {
        self.units
    }

    pub const fn scale(self) -> u8 {
        self.scale
    }

    /// The value in millionths. It does not overflow, because `units` is an `i64`.
    fn millionths(self) -> i128 {
        i128::from(self.units) * 10_i128.pow(u32::from(Self::MAX_SCALE - self.scale))
    }
}

/// The decimal as text without a float, for example `-0.25`: the inverse of the parse of a proposal value.
impl std::fmt::Display for Decimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = if self.units < 0 { "-" } else { "" };
        let digits = self.units.unsigned_abs().to_string();
        let scale = usize::from(self.scale);
        if scale == 0 {
            return write!(f, "{sign}{digits}");
        }
        let digits = format!("{digits:0>width$}", width = scale + 1);
        let (whole, fraction) = digits.split_at(digits.len() - scale);
        write!(f, "{sign}{whole}.{fraction}")
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.millionths().cmp(&other.millionths())
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// An amount of money in the minor unit of its currency, for example centimes for CHF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinorUnits(i64);

impl MinorUnits {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

/// A single value or a range with `min <= max`. A single value has `min == max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range<T> {
    min: T,
    max: T,
}

impl<T: Ord + Copy> Range<T> {
    pub fn new(min: T, max: T) -> Result<Self, ValueError> {
        if min > max {
            return Err(ValueError::RangeOrder);
        }
        Ok(Self { min, max })
    }

    pub fn exact(value: T) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    pub fn min(&self) -> T {
        self.min
    }

    pub fn max(&self) -> T {
        self.max
    }
}

/// The granularity of a date window: what its dates mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Day,
    Week,
    Month,
}

/// A range of dates with `start <= end`, for example "May to June 2030".
/// The same type serves the fact value and the code that computes with the date window (ADR 0049).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateWindow {
    start: civil::Date,
    end: civil::Date,
    granularity: Granularity,
}

impl DateWindow {
    pub fn new(
        start: civil::Date,
        end: civil::Date,
        granularity: Granularity,
    ) -> Result<Self, ValueError> {
        if start > end {
            return Err(ValueError::DateWindowOrder);
        }
        Ok(Self {
            start,
            end,
            granularity,
        })
    }

    pub fn start(&self) -> civil::Date {
        self.start
    }

    pub fn end(&self) -> civil::Date {
        self.end
    }

    pub fn granularity(&self) -> Granularity {
        self.granularity
    }
}

snake_case_key!(
    /// The unit of a quantity field, for example `person_per_day`. The web client shows its label.
    Unit
);

snake_case_key!(
    /// The stable key of one choice of a choice field, for example `static_display`.
    ChoiceKey
);

/// One choice of a choice field: its stable key and its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceValue {
    pub key: ChoiceKey,
    pub label: super::Label,
}

/// A currency: an ISO 4217 code of three capital letters, for example `CHF`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency(String);

impl Currency {
    pub fn parse(code: &str) -> Result<Self, ValueError> {
        if code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            Ok(Self(code.to_owned()))
        } else {
            Err(ValueError::Currency)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kind of record that a reference field points to. Slice 1 has documents and events only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceTarget {
    Document,
    Event,
}

/// The record that a reference value points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceId {
    Document(DocumentId),
    Event(EventId),
}

impl ReferenceId {
    pub fn target(self) -> ReferenceTarget {
        match self {
            Self::Document(_) => ReferenceTarget::Document,
            Self::Event(_) => ReferenceTarget::Event,
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use uuid::Uuid;

    use super::*;
    use crate::facts::{Label, MessageId};

    fn choice_type(keys: &[&str], multiple: bool) -> ValueType {
        ValueType::Choice {
            values: keys
                .iter()
                .map(|key| ChoiceValue {
                    key: ChoiceKey::parse(key).unwrap(),
                    label: Label::Builtin(MessageId::parse(&format!("choice-{key}")).unwrap()),
                })
                .collect(),
            multiple,
        }
    }

    fn choice(keys: &[&str]) -> FactValue {
        FactValue::Choice(
            keys.iter()
                .map(|key| ChoiceKey::parse(key).unwrap())
                .collect(),
        )
    }

    #[test]
    fn rejects_a_date_window_that_ends_before_it_starts() {
        assert_eq!(
            DateWindow::new(date(2030, 6, 1), date(2030, 5, 1), Granularity::Month),
            Err(ValueError::DateWindowOrder)
        );
        let window =
            DateWindow::new(date(2030, 5, 18), date(2030, 5, 18), Granularity::Day).unwrap();
        assert_eq!(window.start(), window.end());
    }

    #[test]
    fn checks_the_granularity_of_a_date_window() {
        let months =
            DateWindow::new(date(2030, 5, 1), date(2030, 6, 30), Granularity::Month).unwrap();
        let value = FactValue::DateWindow(months);
        let any = ValueType::DateWindow { granularity: None };
        let days = ValueType::DateWindow {
            granularity: Some(Granularity::Day),
        };
        assert_eq!(value.check(&any), Ok(()));
        assert_eq!(value.check(&days), Err(ValueError::GranularityMismatch));
    }

    #[test]
    fn rejects_a_choice_outside_the_list() {
        let audience = choice_type(&["public", "members", "invited"], false);
        assert_eq!(choice(&["public"]).check(&audience), Ok(()));
        assert_eq!(
            choice(&["everybody"]).check(&audience),
            Err(ValueError::UnknownChoice)
        );
    }

    #[test]
    fn rejects_several_values_on_a_choice_that_is_not_multiple() {
        let single = choice_type(&["airshow", "catering"], false);
        let multiple = choice_type(&["airshow", "catering"], true);
        let both = choice(&["airshow", "catering"]);
        assert_eq!(both.check(&single), Err(ValueError::SeveralChoices));
        assert_eq!(both.check(&multiple), Ok(()));
    }

    #[test]
    fn rejects_an_empty_or_repeated_choice() {
        let multiple = choice_type(&["airshow", "catering"], true);
        assert_eq!(choice(&[]).check(&multiple), Err(ValueError::NoChoice));
        assert_eq!(
            choice(&["airshow", "airshow"]).check(&multiple),
            Err(ValueError::DuplicateChoice)
        );
    }

    #[test]
    fn rejects_a_value_of_another_type() {
        assert_eq!(
            FactValue::Boolean(true).check(&ValueType::Text),
            Err(ValueError::TypeMismatch)
        );
        let event = FactValue::Reference(ReferenceId::Event(EventId::from_uuid(Uuid::nil())));
        assert_eq!(
            event.check(&ValueType::Reference {
                target: ReferenceTarget::Document
            }),
            Err(ValueError::ReferenceTargetMismatch)
        );
        assert_eq!(
            event.check(&ValueType::Reference {
                target: ReferenceTarget::Event
            }),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_decimal_with_a_scale_above_six() {
        assert_eq!(Decimal::new(1, 7), Err(ValueError::ScaleTooLarge));
        assert!(Decimal::new(1, 6).is_ok());
    }

    #[test]
    fn writes_a_decimal_as_text_without_a_float() {
        let text = |units, scale| Decimal::new(units, scale).unwrap().to_string();
        assert_eq!(text(20_000, 0), "20000");
        assert_eq!(text(15, 1), "1.5");
        assert_eq!(text(150, 2), "1.5");
        assert_eq!(text(-25, 2), "-0.25");
        assert_eq!(text(5, 3), "0.005");
        assert_eq!(text(i64::MIN, 6), "-9223372036854.775808");
    }

    #[test]
    fn compares_decimals_exactly() {
        let one_and_a_half = Decimal::new(15, 1).unwrap();
        assert_eq!(Decimal::new(150, 2).unwrap(), one_and_a_half);
        assert_eq!(Decimal::new(1_500_000, 6).unwrap(), one_and_a_half);
        assert!(Decimal::new(1_500_001, 6).unwrap() > one_and_a_half);
        assert!(Decimal::integer(1) < one_and_a_half);
        assert!(Decimal::new(i64::MIN, 0).unwrap() < Decimal::new(i64::MAX, 6).unwrap());
    }

    #[test]
    fn compares_minor_units_exactly() {
        assert_eq!(MinorUnits::new(8_000_000), MinorUnits::new(8_000_000));
        assert!(MinorUnits::new(1) > MinorUnits::new(0));
        assert_eq!(
            Range::new(MinorUnits::new(2), MinorUnits::new(1)),
            Err(ValueError::RangeOrder)
        );
        let fee = Range::exact(MinorUnits::new(1500));
        assert_eq!(fee.min(), fee.max());
    }

    #[test]
    fn rejects_a_range_whose_minimum_is_above_its_maximum() {
        let low = Decimal::integer(15_000);
        let high = Decimal::integer(25_000);
        assert!(Range::new(low, high).is_ok());
        assert_eq!(Range::new(high, low), Err(ValueError::RangeOrder));
    }

    #[test]
    fn checks_currencies_and_short_texts() {
        assert_eq!(Currency::parse("CHF").unwrap().as_str(), "CHF");
        for code in ["chf", "CH", "CHFX", "C1F"] {
            assert_eq!(Currency::parse(code), Err(ValueError::Currency), "{code:?}");
        }
        assert_eq!(
            ShortText::parse(" military airfield ").unwrap().as_str(),
            "military airfield"
        );
        assert_eq!(
            ShortText::parse(""),
            Err(ValueError::Text(TextError::Empty))
        );
    }
}
