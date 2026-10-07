//! The OpenAPI document and the problem catalog (ADRs 0017 and 0037).

use std::collections::BTreeMap;

use tada_app::problem::ProblemCode;
use utoipa::openapi::extensions::Extensions;
use utoipa::openapi::{Info, License, OpenApi};

use crate::problem;

/// The extension that lists the problem codes of an operation (ADR 0037).
pub const PROBLEM_CODES_EXTENSION: &str = "x-tada-problem-codes";

/// The codes of each operation that needs a member caller.
pub(crate) const AUTHENTICATED: &[ProblemCode] = &[
    ProblemCode::Unauthenticated,
    ProblemCode::OrganizationRequired,
    ProblemCode::Unavailable,
    ProblemCode::Internal,
];
/// The codes of each operation with a JSON body.
pub(crate) const JSON_BODY: &[ProblemCode] = &[
    ProblemCode::MalformedRequest,
    ProblemCode::UnsupportedMediaType,
    ProblemCode::PayloadTooLarge,
];
/// The codes of each state-changing operation: the `Origin` check (ADR 0008).
const STATE_CHANGE: &[ProblemCode] = &[ProblemCode::Forbidden];
/// The codes of each operation with query parameters.
pub(crate) const QUERY: &[ProblemCode] = &[ProblemCode::MalformedRequest];

/// The sorted union of code lists.
pub(crate) fn codes(lists: &[&[ProblemCode]]) -> Vec<ProblemCode> {
    let mut codes: Vec<ProblemCode> = lists.iter().flat_map(|list| list.iter().copied()).collect();
    codes.sort();
    codes.dedup();
    codes
}

/// Completes the generated document: the general information and the problem codes of each operation.
///
/// # Panics
///
/// If an operation has no entry in `problem_codes`. A test runs this for all operations.
pub(crate) fn complete(
    mut document: OpenApi,
    problem_codes: &BTreeMap<&str, Vec<ProblemCode>>,
) -> OpenApi {
    let mut info = Info::new("tada", "1");
    info.description =
        Some("The tada HTTP API. The list of problem codes of each operation is open.".to_owned());
    info.license = Some(License::new("Apache-2.0"));
    document.info = info;

    for item in document.paths.paths.values_mut() {
        // The `Origin` check rejects a state change before its handler runs (ADR 0008).
        let operations = [
            (&mut item.get, &[][..]),
            (&mut item.put, STATE_CHANGE),
            (&mut item.post, STATE_CHANGE),
            (&mut item.delete, STATE_CHANGE),
            (&mut item.patch, STATE_CHANGE),
        ];
        for (operation, method_codes) in operations {
            let Some(operation) = operation else {
                continue;
            };
            let id = operation.operation_id.as_deref().unwrap_or_default();
            let Some(handler_codes) = problem_codes.get(id) else {
                panic!("the operation {id} has no problem codes");
            };
            let codes = codes(&[handler_codes, method_codes]);
            let names: Vec<&str> = codes.iter().map(|code| code.as_str()).collect();
            let extensions = operation.extensions.get_or_insert_with(Extensions::default);
            extensions.insert(PROBLEM_CODES_EXTENSION.to_owned(), names.into());
        }
    }
    document
}

/// The catalog of all problem codes as Markdown, for `doc/problems.md`.
pub fn problem_catalog() -> String {
    let mut rows = vec![["Code".to_owned(), "Status".to_owned(), "Meaning".to_owned()]];
    for code in ProblemCode::ALL {
        let name = code.as_str();
        rows.push([
            format!("<a id=\"{name}\"></a>`{name}`"),
            problem::status(code).as_u16().to_string(),
            code.meaning().to_owned(),
        ]);
    }
    let mut out = String::from(
        "# Problem codes\n\n\
         This catalog lists the stable problem codes of the tada API (ADR 0037).\n\
         The `type` of each error response links to its entry here.\n\
         The command `tada problems` writes this file. Do not change it by hand.\n\n\
         A code never changes its meaning. A code that is no longer used stays reserved.\n\n",
    );
    out.push_str(&table(&rows));
    out
}

/// A Markdown table with padded columns, in the layout that mdformat keeps.
fn table(rows: &[[String; 3]]) -> String {
    let mut widths = [3; 3];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |cells: &[String]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        format!("| {} |\n", padded.join(" | "))
    };
    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        out.push_str(&line(row));
        if index == 0 {
            let rules: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
            out.push_str(&line(&rules));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_operation_lists_its_problem_codes() {
        let document = crate::openapi();
        let mut operations = 0;
        for item in document.paths.paths.values() {
            for operation in [&item.get, &item.put, &item.post, &item.delete, &item.patch]
                .into_iter()
                .flatten()
            {
                let extensions = operation.extensions.as_ref().unwrap();
                assert!(extensions.contains_key(PROBLEM_CODES_EXTENSION));
                operations += 1;
            }
        }
        assert_eq!(
            operations,
            crate::problem_codes().len(),
            "a code list without an operation"
        );
    }

    #[test]
    fn each_state_change_lists_the_origin_check() {
        let document = crate::openapi();
        let codes = |operation: &utoipa::openapi::path::Operation| {
            operation.extensions.as_ref().unwrap()[PROBLEM_CODES_EXTENSION].clone()
        };
        for item in document.paths.paths.values() {
            if let Some(post) = &item.post {
                assert!(
                    codes(post)
                        .as_array()
                        .unwrap()
                        .contains(&"forbidden".into())
                );
            }
        }
        let events = &document.paths.paths["/api/v1/events"];
        let list = codes(events.get.as_ref().unwrap());
        assert!(!list.as_array().unwrap().contains(&"forbidden".into()));
    }

    #[test]
    fn the_catalog_lists_each_code_once() {
        let catalog = problem_catalog();
        for code in ProblemCode::ALL {
            assert_eq!(
                catalog
                    .matches(&format!("id=\"{}\"", code.as_str()))
                    .count(),
                1
            );
        }
    }
}
