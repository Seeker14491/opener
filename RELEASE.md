# Release Procedure

This project uses `cargo-release` integrated with a GitHub Actions workflow to automate releases.

## Prerequisites

1. Ensure the `CARGO_REGISTRY_TOKEN` secret is configured in the GitHub repository settings (Settings -> Secrets and variables -> Actions).

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

1. Checkout the latest code from the main branch.
2. Set up the Rust environment.
3. Install `cargo-release`.
4. Run tests (`cargo test --all-features`) within the `opener` directory.
5. Check code formatting (`cargo fmt -- --check`) within the `opener` directory.
6. If checks pass, `cargo-release` (using the configuration in `opener/Cargo.toml`) will:
    - Update the `## [Unreleased]` section in `CHANGELOG.md` to `## [your-new-version] - YYYY-MM-DD` and add a new `## [Unreleased]` section above it.
    - Update the `version` in `opener/Cargo.toml` to the version you provided.
    - Commit these changes (changelog and `Cargo.toml` update).
    - Create a Git tag for the new version (e.g., `v0.8.0`).
    - Push the commit and the new tag to the repository.
    - Publish the `opener` crate to `crates.io`.
