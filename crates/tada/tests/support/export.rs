//! The structured export of `tada export` and the files that it writes (ADR 0059).

use std::collections::BTreeMap;
use std::path::Path;

use tada::export::{ExportCommand, execute};
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::testing::TestGarage;
use tada_app::domain::identity::OrganizationSlug;
use tada_store_pg::testing::TestDatabase;

/// Exports the organization `slug` into the folder `output`, which must not exist or be empty.
pub async fn export(
    test: &TestDatabase,
    garage: &TestGarage,
    slug: &str,
    output: &Path,
) -> tada_app::export::ExportSummary {
    execute(
        &test.database,
        &garage.storage,
        &SystemClock,
        ExportCommand {
            organization_slug: OrganizationSlug::parse(slug).unwrap(),
            output: output.to_owned(),
        },
    )
    .await
    .unwrap()
}

/// All files of the export by their path relative to the export.
pub fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                files.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    files
}

/// True if `haystack` holds the bytes of `needle`.
pub fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}
