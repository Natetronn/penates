//! Sets `PENATES_BUILD_VERSION`, the version shown to people: the plain version for a release build
//! (the commit is tagged `v<version>`), else `<version>+<commit>` so a development build is never
//! mistaken for a release. Without git (a source tarball) it is the plain version.

use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // The Release workflow builds a tag push; GitHub says which tag, even in a shallow checkout.
    let ci_release = std::env::var("GITHUB_REF_TYPE").is_ok_and(|t| t == "tag")
        && std::env::var("GITHUB_REF_NAME").is_ok_and(|n| n == format!("v{version}"));
    let shown = match git(&root, &["rev-parse", "--short", "HEAD"]) {
        _ if ci_release => version.clone(),
        None => version.clone(),
        Some(_) if git(&root, &["tag", "--points-at", "HEAD"]).is_some_and(|t| t.lines().any(|t| t == format!("v{version}"))) => {
            version.clone()
        }
        Some(rev) => format!("{version}+{rev}"),
    };
    println!("cargo:rustc-env=PENATES_BUILD_VERSION={shown}");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_TYPE");
    // Rebuild when the commit or the tags change.
    for p in [".git/HEAD", ".git/refs/heads", ".git/refs/tags", ".git/packed-refs"] {
        println!("cargo:rerun-if-changed={}", root.join(p).display());
    }
}
