//! The draft lint of ADR 0051: deterministic, non-blocking warnings for the review.

use pulldown_cmark::{Event, Tag, TagEnd};

use super::{is_tada_link, normalize_line_endings, parser};

/// What a lint warning found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LintKind {
    /// A number outside a `tada:` link.
    Number,
    /// A date outside a `tada:` link.
    Date,
    /// An amount of money outside a `tada:` link.
    Money,
    /// A raw HTML block or inline HTML. The draft shows it as text.
    RawHtml,
}

/// A lint warning on a line of the draft. The first line is 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LintWarning {
    pub line: u32,
    pub kind: LintKind,
}

/// Lints a draft. The result has one warning for each line and kind, ordered by line and kind.
pub fn lint(markdown: &str) -> Vec<LintWarning> {
    let markdown = normalize_line_endings(markdown);
    let lines = LineIndex::new(&markdown);
    let mut warnings = Vec::new();
    let mut text = TextRun::default();
    let mut in_tada_link = false;
    for (event, range) in parser(&markdown).into_offset_iter() {
        if let Event::Text(content) | Event::Code(content) = &event
            && !in_tada_link
        {
            text.push(range.start, content);
            continue;
        }
        text.check(&lines, &mut warnings);
        match &event {
            Event::Start(Tag::Link { dest_url, .. }) => in_tada_link = is_tada_link(dest_url),
            Event::End(TagEnd::Link) => in_tada_link = false,
            Event::Start(Tag::HtmlBlock) | Event::InlineHtml(_) => warnings.push(LintWarning {
                line: lines.line(range.start),
                kind: LintKind::RawHtml,
            }),
            _ => {}
        }
    }
    text.check(&lines, &mut warnings);
    warnings.sort_unstable();
    warnings.dedup();
    warnings
}

/// The line numbers of byte offsets in the draft.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(markdown: &str) -> Self {
        let starts = std::iter::once(0)
            .chain(markdown.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        Self { starts }
    }

    fn line(&self, offset: usize) -> u32 {
        let line = self.starts.partition_point(|&start| start <= offset);
        u32::try_from(line).unwrap_or(u32::MAX)
    }
}

/// Text outside `tada:` links that the parser gives in several parts, joined for the check.
#[derive(Default)]
struct TextRun {
    start: usize,
    content: String,
}

impl TextRun {
    fn push(&mut self, offset: usize, content: &str) {
        if self.content.is_empty() {
            self.start = offset;
        }
        self.content.push_str(content);
    }

    /// Adds the warnings of the text and empties the run.
    fn check(&mut self, lines: &LineIndex, warnings: &mut Vec<LintWarning>) {
        let first_line = lines.line(self.start);
        for (offset, kind) in find_values(&self.content) {
            let line_breaks = self.content[..offset].matches('\n').count();
            let line = first_line.saturating_add(u32::try_from(line_breaks).unwrap_or(u32::MAX));
            warnings.push(LintWarning { line, kind });
        }
        self.content.clear();
    }
}

/// Characters between two digits that keep a number together: `80'000`, `1.5`, `18.5.2030`.
const NUMBER_SEPARATORS: [char; 6] = ['\'', '’', '.', ',', '-', '/'];

/// German and English month names and their abbreviations, in lowercase.
const MONTHS: [&str; 36] = [
    "januar",
    "februar",
    "märz",
    "maerz",
    "april",
    "mai",
    "juni",
    "juli",
    "august",
    "september",
    "oktober",
    "november",
    "dezember",
    "january",
    "february",
    "march",
    "may",
    "june",
    "july",
    "october",
    "december",
    "jan",
    "feb",
    "mär",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "okt",
    "oct",
    "nov",
    "dez",
    "dec",
];

/// Currency names and symbols before or after an amount, without a final period.
const CURRENCIES: [&str; 10] = [
    "CHF", "Fr", "SFr", "EUR", "USD", "GBP", "Franken", "€", "$", "£",
];

/// Finds numbers, dates and amounts of money in a text.
/// Gives the byte offset of each find. A date or an amount gives one find, not one per number.
fn find_values(text: &str) -> Vec<(usize, LintKind)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let scanner = Scanner { chars: &chars };
    let mut finds = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if scanner.is_digit(index) {
            let (kind, next) = scanner.classify(index);
            finds.push((chars[index].0, kind));
            index = next;
        } else {
            index += 1;
        }
    }
    finds
}

/// Reads the characters around a number. Indexes count characters, not bytes.
struct Scanner<'a> {
    chars: &'a [(usize, char)],
}

impl Scanner<'_> {
    fn char(&self, index: usize) -> Option<char> {
        self.chars.get(index).map(|&(_, c)| c)
    }

    fn is_digit(&self, index: usize) -> bool {
        self.char(index).is_some_and(|c| c.is_ascii_digit())
    }

    fn is_space(&self, index: usize) -> bool {
        self.char(index).is_some_and(char::is_whitespace)
    }

    /// Classifies the number that starts at `start`. Gives the kind and the index after the find.
    fn classify(&self, start: usize) -> (LintKind, usize) {
        let end = self.number_end(start);
        let number: String = self.chars[start..end].iter().map(|&(_, c)| c).collect();
        if is_numeric_date(&number) {
            return (LintKind::Date, end);
        }
        if number.len() <= 2
            && number.bytes().all(|b| b.is_ascii_digit())
            && let Some(next) = self.month_after(end)
        {
            return (LintKind::Date, next);
        }
        if is_month(&self.word_before(start)) {
            return (LintKind::Date, end);
        }
        if is_currency(&self.word_before(start)) {
            return (LintKind::Money, end);
        }
        if let Some(next) = self.currency_after(end) {
            return (LintKind::Money, next);
        }
        (LintKind::Number, end)
    }

    fn number_end(&self, start: usize) -> usize {
        let mut end = start + 1;
        loop {
            if self.is_digit(end) {
                end += 1;
            } else if self
                .char(end)
                .is_some_and(|c| NUMBER_SEPARATORS.contains(&c))
                && self.is_digit(end + 1)
            {
                end += 2;
            } else {
                return end;
            }
        }
    }

    /// The word before `index`, after at least one space: `CHF 80`, `Mai 2030`. Also a
    /// currency symbol directly before the number: `€12`.
    fn word_before(&self, index: usize) -> String {
        let mut end = index;
        while end > 0 && self.is_space(end - 1) {
            end -= 1;
        }
        let mut start = end;
        while start > 0 && !self.is_space(start - 1) && !self.is_digit(start - 1) {
            start -= 1;
        }
        self.chars[start..end]
            .iter()
            .map(|&(_, c)| c)
            .skip_while(|c| !c.is_alphanumeric() && !is_currency(&c.to_string()))
            .collect()
    }

    /// The word that starts at `index`, and the index after it.
    fn word_at(&self, index: usize) -> (String, usize) {
        let mut end = index;
        while self.char(end).is_some_and(char::is_alphabetic) {
            end += 1;
        }
        let word = self.chars[index..end].iter().map(|&(_, c)| c).collect();
        (word, end)
    }

    /// A month name after a day: `18. Mai 2030`, `3 März`. Gives the index after the date.
    fn month_after(&self, end: usize) -> Option<usize> {
        let mut index = end;
        if self.char(index) == Some('.') {
            index += 1;
        }
        if !self.is_space(index) {
            return None;
        }
        while self.is_space(index) {
            index += 1;
        }
        let (word, mut next) = self.word_at(index);
        if !is_month(&word) {
            return None;
        }
        if self.char(next) == Some('.') {
            next += 1;
        }
        let mut year = next;
        while self.is_space(year) {
            year += 1;
        }
        if year > next && (year..year + 4).all(|i| self.is_digit(i)) && !self.is_digit(year + 4) {
            next = year + 4;
        }
        Some(next)
    }

    /// A currency after an amount: `80.–`, `80.- Franken`, `12 CHF`. Gives the index after it.
    fn currency_after(&self, end: usize) -> Option<usize> {
        if self.char(end) == Some('.') && matches!(self.char(end + 1), Some('-' | '–' | '—')) {
            return Some(end + 2);
        }
        let mut index = end;
        while self.is_space(index) {
            index += 1;
        }
        let mut word_end = index;
        while self
            .char(word_end)
            .is_some_and(|c| c.is_alphabetic() || is_currency(&c.to_string()))
        {
            word_end += 1;
        }
        let word: String = self.chars[index..word_end]
            .iter()
            .map(|&(_, c)| c)
            .collect();
        is_currency(&word).then_some(word_end)
    }
}

fn is_month(word: &str) -> bool {
    MONTHS.contains(&word.trim_end_matches('.').to_lowercase().as_str())
}

fn is_currency(word: &str) -> bool {
    CURRENCIES.contains(&word.trim_end_matches('.'))
}

/// `18.5.2030`, `18.05.30`, `18/5/2030` or `2030-05-18`.
fn is_numeric_date(number: &str) -> bool {
    ['.', '/', '-'].into_iter().any(|separator| {
        let parts: Vec<usize> = number.split(separator).map(str::len).collect();
        let digits_only = number.chars().all(|c| c.is_ascii_digit() || c == separator);
        digits_only
            && match (separator, parts.as_slice()) {
                ('-', [4, 2, 2]) => true,
                ('-', _) => false,
                (_, [day, month, year]) => {
                    (1..=2).contains(day) && (1..=2).contains(month) && matches!(year, 2 | 4)
                }
                _ => false,
            }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACT: &str = "0190f3a2-7b1c-7d4e-8f00-000000000001";
    const SOURCE: &str = "0190f3a2-7b1c-7d4e-8f00-0000000000a1";

    fn warning(line: u32, kind: LintKind) -> LintWarning {
        LintWarning { line, kind }
    }

    #[test]
    fn finds_a_date_and_money_outside_links() {
        let warnings = lint("Das Fest ist am 18. Mai 2030.\nDas Budget beträgt CHF 80'000.\n");

        assert_eq!(
            warnings,
            vec![warning(1, LintKind::Date), warning(2, LintKind::Money)]
        );
    }

    #[test]
    fn finds_nothing_inside_tada_links() {
        let markdown = format!(
            "Das Fest ist [am 18. Mai 2030](tada:source/{SOURCE}#0-14), am [](tada:fact/{FACT}?v=2).\n\
             Das Budget beträgt [CHF 80'000](tada:source/{SOURCE}#20-30).\n"
        );

        assert_eq!(lint(&markdown), vec![]);
    }

    #[test]
    fn checks_the_text_of_other_links() {
        let warnings = lint("Siehe [CHF 80'000](https://example.org).\n");

        assert_eq!(warnings, vec![warning(1, LintKind::Money)]);
    }

    #[test]
    fn finds_numbers() {
        assert_eq!(
            lint("Wir erwarten 300 Gäste.\n"),
            vec![warning(1, LintKind::Number)]
        );
        assert_eq!(
            lint("Es kommen 1'200 Gäste.\n"),
            vec![warning(1, LintKind::Number)]
        );
        assert_eq!(lint("Ohne Zahl.\n"), vec![]);
    }

    #[test]
    fn finds_dates_in_common_forms() {
        for text in [
            "am 18. Mai 2030",
            "am 18.05.2030",
            "am 18.5.30",
            "am 2030-05-18",
            "am 3 März",
            "on May 18",
            "im Mai 2030",
        ] {
            assert_eq!(lint(text), vec![warning(1, LintKind::Date)], "{text}");
        }
    }

    #[test]
    fn finds_money_in_common_forms() {
        for text in [
            "CHF 80'000",
            "CHF 80’000.50",
            "Fr. 80.–",
            "80.- Franken",
            "EUR 12",
            "€12",
            "12 CHF",
        ] {
            assert_eq!(lint(text), vec![warning(1, LintKind::Money)], "{text}");
        }
    }

    #[test]
    fn finds_raw_html_blocks_and_inline_html() {
        let warnings = lint("<script>alert(1)</script>\n\nText <b>fett</b>.\n");

        assert_eq!(
            warnings,
            vec![warning(1, LintKind::RawHtml), warning(3, LintKind::RawHtml)]
        );
    }

    #[test]
    fn gives_one_warning_per_line_and_kind_in_order() {
        let warnings = lint("CHF 5, 7 und 9 am 1. Juni <i>x</i>\n\n| a |\n| - |\n| 4 |\n");

        assert_eq!(
            warnings,
            vec![
                warning(1, LintKind::Number),
                warning(1, LintKind::Date),
                warning(1, LintKind::Money),
                warning(1, LintKind::RawHtml),
                warning(5, LintKind::Number),
            ]
        );
    }

    #[test]
    fn counts_lines_after_normalizing_line_endings() {
        let lf = "Titel\n\nDas Budget beträgt CHF 80'000.\n";

        assert_eq!(lint(lf), vec![warning(3, LintKind::Money)]);
        assert_eq!(lint(&lf.replace('\n', "\r\n")), lint(lf));
        assert_eq!(lint(&lf.replace('\n', "\r")), lint(lf));
    }

    #[test]
    fn counts_lines_inside_code_blocks() {
        let warnings = lint("```\nText\n42\n```\n");

        assert_eq!(warnings, vec![warning(3, LintKind::Number)]);
    }
}
