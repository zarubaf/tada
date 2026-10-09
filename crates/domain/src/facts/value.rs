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
    #[error("the start of a range is after its end")]
    RangeOrder,
    #[error("the start of a date window is after its end")]
    DateWindowOrder,
    #[error("a currency is an ISO 4217 code of three capital letters")]
    Currency,
    #[error("the value type has no text form")]
    NoTextForm,
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

impl FactValue {
    /// Reads the short text form that a member types in a chat, for the value type of the field:
    ///
    /// - `text`: the text itself;
    /// - `boolean`: `ja` or `nein`;
    /// - `quantity`: a decimal, for example `20000` or `1.5`;
    /// - `money`: the currency of the field and a decimal amount, for example `CHF 80000`;
    /// - `date`: `2030-05-18` or `18.05.2030`;
    /// - `date_window`: two dates or two months, for example `2030-05..2030-06` or `2030-05-18..2030-05-19`;
    /// - `choice`: the key of the choice.
    ///
    /// A `reference` has no text form. The value must also pass `check` for `value_type`.
    pub fn parse_text(text: &str, value_type: &ValueType) -> Result<Valued, ValueError> {
        let text = text.trim();
        let value = match value_type {
            ValueType::Text => Self::Text(ShortText::parse(text)?),
            ValueType::Boolean => match text.to_lowercase().as_str() {
                "ja" => Self::Boolean(true),
                "nein" => Self::Boolean(false),
                _ => return Err(ValueError::TypeMismatch),
            },
            ValueType::Quantity { .. } => Self::Quantity(Range::exact(Decimal::parse(text)?)),
            ValueType::Money { currency } => parse_money(text, currency)?,
            ValueType::Date => Self::Date(parse_date(text)?),
            ValueType::DateWindow { .. } => parse_window(text)?,
            ValueType::Choice { .. } => Self::Choice(vec![ChoiceKey::parse(text)?]),
            ValueType::Reference { .. } => return Err(ValueError::NoTextForm),
        };
        value.check(value_type)?;
        Ok(Valued {
            value,
            approximate: false,
        })
    }
}

/// The digits after the decimal point of a currency amount: CHF and EUR have centimes and cents.
const MONEY_SCALE: u8 = 2;

fn parse_money(text: &str, currency: &Currency) -> Result<FactValue, ValueError> {
    let (code, amount) = text.split_once(' ').ok_or(ValueError::TypeMismatch)?;
    if Currency::parse(code)? != *currency {
        return Err(ValueError::Currency);
    }
    let amount = Decimal::parse(amount.trim())?;
    if amount.scale() > MONEY_SCALE {
        return Err(ValueError::ScaleTooLarge);
    }
    let minor = amount
        .units()
        .checked_mul(10_i64.pow(u32::from(MONEY_SCALE - amount.scale())))
        .ok_or(ValueError::TypeMismatch)?;
    Ok(FactValue::Money(Range::exact(MinorUnits::new(minor))))
}

/// A date as `2030-05-18` or `18.05.2030`.
fn parse_date(text: &str) -> Result<civil::Date, ValueError> {
    if let Ok(date) = text.parse::<civil::Date>() {
        return Ok(date);
    }
    let mut parts = text.split('.');
    let (Some(day), Some(month), Some(year), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ValueError::TypeMismatch);
    };
    let invalid = || ValueError::TypeMismatch;
    civil::Date::new(
        year.parse().map_err(|_| invalid())?,
        month.parse().map_err(|_| invalid())?,
        day.parse().map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())
}

/// A month as `2030-05`: its first and its last day.
fn parse_month(text: &str) -> Option<(civil::Date, civil::Date)> {
    let (year, month) = text.split_once('-')?;
    if year.len() != 4 || month.len() != 2 {
        return None;
    }
    let first = civil::Date::new(year.parse().ok()?, month.parse().ok()?, 1).ok()?;
    Some((first, first.last_of_month()))
}

/// Two dates or two months, `start..end`. Both sides have the same form, which gives the granularity.
fn parse_window(text: &str) -> Result<FactValue, ValueError> {
    let (start, end) = text.split_once("..").ok_or(ValueError::TypeMismatch)?;
    let (start, end) = (start.trim(), end.trim());
    let window = match (parse_month(start), parse_month(end)) {
        (Some((start, _)), Some((_, end))) => DateWindow::new(start, end, Granularity::Month)?,
        (None, None) => DateWindow::new(parse_date(start)?, parse_date(end)?, Granularity::Day)?,
        _ => return Err(ValueError::TypeMismatch),
    };
    Ok(FactValue::DateWindow(window))
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
#[derive(Clone, PartialEq, Eq)]
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

/// A short text can hold a name or contact details, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for ShortText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShortText({} characters)", self.0.chars().count())
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

    /// Reads a decimal such as `20000`, `1.5` or `-0.25`. It has no exponent, no `+` and no spaces.
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let invalid = ValueError::TypeMismatch;
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty() || !all_digits(whole) || !all_digits(fraction) || digits.ends_with('.')
        {
            return Err(invalid);
        }
        let scale = u8::try_from(fraction.len()).map_err(|_| ValueError::ScaleTooLarge)?;
        if scale > Self::MAX_SCALE {
            return Err(ValueError::ScaleTooLarge);
        }
        let units: i64 = format!("{whole}{fraction}").parse().map_err(|_| invalid)?;
        Self::new(if negative { -units } else { units }, scale)
    }

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

    #[test]
    fn debug_of_a_short_text_and_of_the_types_that_wrap_it_hides_the_text() {
        let text = ShortText::parse("Erika Muster").unwrap();
        let valued = Valued {
            value: FactValue::Text(text.clone()),
            approximate: false,
        };
        let choice = ChoiceValue {
            key: ChoiceKey::parse("pick").unwrap(),
            label: Label::Text(text.clone()),
        };
        assert_eq!(format!("{text:?}"), "ShortText(12 characters)");
        for shown in [format!("{valued:?}"), format!("{choice:?}")] {
            assert!(
                !shown.contains("Erika") && !shown.contains("Muster"),
                "{shown}"
            );
        }
    }

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

    fn parse(text: &str, value_type: &ValueType) -> Result<FactValue, ValueError> {
        FactValue::parse_text(text, value_type).map(|valued| {
            assert!(!valued.approximate);
            valued.value
        })
    }

    #[test]
    fn parses_text_boolean_and_quantity() {
        assert_eq!(
            parse(" Flugplatz Testwil ", &ValueType::Text),
            Ok(FactValue::Text(
                ShortText::parse("Flugplatz Testwil").unwrap()
            ))
        );
        assert_eq!(
            parse("Ja", &ValueType::Boolean),
            Ok(FactValue::Boolean(true))
        );
        assert_eq!(
            parse("nein", &ValueType::Boolean),
            Ok(FactValue::Boolean(false))
        );
        assert_eq!(
            parse("vielleicht", &ValueType::Boolean),
            Err(ValueError::TypeMismatch)
        );
        let people = ValueType::Quantity {
            unit: Unit::parse("person").unwrap(),
        };
        assert_eq!(
            parse("20000", &people),
            Ok(FactValue::Quantity(Range::exact(Decimal::integer(20_000))))
        );
        assert_eq!(
            parse("1.50", &people),
            Ok(FactValue::Quantity(Range::exact(
                Decimal::new(15, 1).unwrap()
            )))
        );
        for text in ["", "1,5", "1e3", "+1", "1.", ".5", "20 000"] {
            assert_eq!(
                parse(text, &people),
                Err(ValueError::TypeMismatch),
                "{text:?}"
            );
        }
        assert_eq!(parse("0.0000001", &people), Err(ValueError::ScaleTooLarge));
    }

    #[test]
    fn parses_money_in_the_currency_of_the_field() {
        let chf = ValueType::Money {
            currency: Currency::parse("CHF").unwrap(),
        };
        let amount = |minor| Ok(FactValue::Money(Range::exact(MinorUnits::new(minor))));
        assert_eq!(parse("CHF 80000", &chf), amount(8_000_000));
        assert_eq!(parse("CHF 15.5", &chf), amount(1_550));
        assert_eq!(parse("CHF 0.05", &chf), amount(5));
        assert_eq!(parse("EUR 80000", &chf), Err(ValueError::Currency));
        assert_eq!(parse("chf 80000", &chf), Err(ValueError::Currency));
        assert_eq!(parse("CHF 1.005", &chf), Err(ValueError::ScaleTooLarge));
        assert_eq!(parse("80000", &chf), Err(ValueError::TypeMismatch));
        assert_eq!(parse("CHF lots", &chf), Err(ValueError::TypeMismatch));
        assert_eq!(
            parse("CHF 99999999999999999", &chf),
            Err(ValueError::TypeMismatch)
        );
    }

    #[test]
    fn parses_a_date_in_iso_or_swiss_form() {
        let expected = Ok(FactValue::Date(date(2030, 5, 18)));
        assert_eq!(parse("2030-05-18", &ValueType::Date), expected);
        assert_eq!(parse("18.05.2030", &ValueType::Date), expected);
        for text in ["2030-02-30", "31.04.2030", "18.05.", "18-05-2030", "morgen"] {
            assert_eq!(
                parse(text, &ValueType::Date),
                Err(ValueError::TypeMismatch),
                "{text:?}"
            );
        }
    }

    #[test]
    fn parses_a_date_window_of_months_or_days() {
        let any = ValueType::DateWindow { granularity: None };
        let window = |start, end, granularity| {
            Ok(FactValue::DateWindow(
                DateWindow::new(start, end, granularity).unwrap(),
            ))
        };
        assert_eq!(
            parse("2030-05..2030-06", &any),
            window(date(2030, 5, 1), date(2030, 6, 30), Granularity::Month)
        );
        assert_eq!(
            parse("2030-02..2030-02", &any),
            window(date(2030, 2, 1), date(2030, 2, 28), Granularity::Month)
        );
        assert_eq!(
            parse("2030-05-18..2030-05-19", &any),
            window(date(2030, 5, 18), date(2030, 5, 19), Granularity::Day)
        );
        assert_eq!(
            parse("18.05.2030..19.05.2030", &any),
            window(date(2030, 5, 18), date(2030, 5, 19), Granularity::Day)
        );
        assert_eq!(
            parse("2030-06..2030-05", &any),
            Err(ValueError::DateWindowOrder)
        );
        for text in ["2030-05", "2030-05..2030-06-01", "2030-13..2030-14", "a..b"] {
            assert_eq!(parse(text, &any), Err(ValueError::TypeMismatch), "{text:?}");
        }
        let days = ValueType::DateWindow {
            granularity: Some(Granularity::Day),
        };
        assert_eq!(
            parse("2030-05..2030-06", &days),
            Err(ValueError::GranularityMismatch)
        );
    }

    #[test]
    fn parses_a_choice_key_from_the_list_and_rejects_a_reference() {
        let audience = choice_type(&["public", "members"], false);
        assert_eq!(parse("public", &audience), Ok(choice(&["public"])));
        assert_eq!(
            parse("everybody", &audience),
            Err(ValueError::UnknownChoice)
        );
        assert!(matches!(
            parse("Public Day", &audience),
            Err(ValueError::Key(_))
        ));
        let event = ValueType::Reference {
            target: ReferenceTarget::Event,
        };
        assert_eq!(parse("FLY28", &event), Err(ValueError::NoTextForm));
    }
}
