// Rebuild when migrations change to catch new files in incremental builds.
// The sqlx::migrate!() macro embeds migrations at compile time, but without
// this hook, adding a new migration file doesn't trigger a rebuild.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
