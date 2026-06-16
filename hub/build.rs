// Stamps the build's git SHA into the binary as BUILD_SHA (read with
// env!("BUILD_SHA")). CI passes BUILD_SHA explicitly; locally we fall back to
// `git rev-parse HEAD`. The deployed /version.json carries the same value, and
// the app compares the two to detect that a new version has shipped.
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_SHA");

    let sha = std::env::var("BUILD_SHA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "dev".to_string());

    println!("cargo:rustc-env=BUILD_SHA={sha}");
}
