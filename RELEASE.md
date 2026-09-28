# Release Procedure

This project uses `cargo-release` integrated with a GitHub Actions workflow to automate releases.

## Prerequisites

1. Ensure [trusted publishing](https://crates.io/docs/trusted-publishing) is configured for the `opener` crate on crates.io (crate Settings -> Trusted Publishing -> Add), with repository owner `Seeker14491`, repository name `opener`, and workflow filename `release.yml`. The workflow gets a short-lived crates.io token this way, so no `CARGO_REGISTRY_TOKEN` secret is needed.

## Steps to Create a Release

1. **Update Changelog**:
    - In `CHANGELOG.md`, add all new changes under the `## [Unreleased]` section.
    - Commit and push these changes to master.

2. **Trigger the Release Workflow**:
    - Navigate to the **Actions** tab in the GitHub repository.
    - In the left sidebar, under "Workflows", click on **"Release Crate"**.
    - Click the **"Run workflow"** dropdown button (usually on the right side of the page).
    - In the **"version"** input field, type the exact semantic version for the new release (e.g., `0.8.0`, `1.2.3`).
    - Click the green **"Run workflow"** button to start the release process.

## What the Workflow Does

The "Release Crate" workflow will perform the following actions:

1. Check that it was started from `master` and that the version looks like `1.2.3`.
2. Run the CI workflow on that commit: tests and Clippy on Linux, Windows and macOS, Clippy for FreeBSD and 32-bit Windows, a docs.rs-style documentation build, and a formatting check.
3. If CI passes, set up the Rust environment, install `cargo-release`, and get a crates.io token through trusted publishing.
4. `cargo-release` (using the configuration in `release.toml` and `opener/release.toml`) will then:
    - Update the `## [Unreleased]` section in `CHANGELOG.md` to `## [your-new-version] - YYYY-MM-DD` and add a new `## [Unreleased]` section above it.
    - Update the `version` in `opener/Cargo.toml` to the version you provided.
    - Commit these changes (changelog and `Cargo.toml` update).
    - Create a Git tag for the new version (e.g., `v0.8.0`).
    - Push the commit and the new tag to the repository.
    - Publish the `opener` crate to `crates.io`.
