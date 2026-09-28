# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/) and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Changed

- On WSL, `open()` now uses the Windows shell via PowerShell instead of the discontinued `wslview`, and reports when Windows cannot open the target. `xdg-open` is used only if PowerShell is unavailable.
- Updated the bundled `xdg-open` to xdg-utils 1.2.1, with a fix so that file URLs for other hosts aren't treated as local relative paths.
- Clarified that `open()` and `open_browser()` do not wait for the opened application to exit, but may block while preparing or dispatching the launch.
- Documented that a program installed with Flatpak, such as a browser, is given access only to the opened file, and how users can let it read the files that file refers to.
- `OpenError::Io` now displays the underlying error's message instead of just "IO error". Its `source()` is now that error's own source, so error reports don't repeat the message.

### Fixed

- On Linux and other platforms that use `xdg-open`, `open()` now returns an error for a path that doesn't exist, instead of `Ok(())`. Other `xdg-open` failures are still not reported, since it may not exit until the opened application does.
- On Windows and WSL, `open_browser()` now correctly handles percent-encoded file URLs and preserves queries and fragments when `BROWSER` is unset.
- On Windows, `open()` now handles percent-encoded non-ASCII characters in file URLs.
- On WSL, `open_browser()` no longer mangles URLs when `BROWSER` names a Windows executable.
- `open_browser()` no longer closes the stderr pipe of the `BROWSER` program, which could kill it with `SIGPIPE`.
- The bundled `xdg-open` script now handles paths that start with `-`.
- On Linux and other Unix platforms, programs launched without waiting, such as `xdg-open` and the `BROWSER` program, no longer remain as zombie processes after they exit.

## [0.8.5] - 2026-06-11

## [0.8.4] - 2026-01-25

## [0.8.3] - 2025-09-04

## [0.8.2] - 2025-05-28

## [0.8.1] - 2025-05-21
## [0.8.0] - 2025-05-21

### Removed

- `dbus-vendored` feature, as we've moved from the `dbus` crate to the pure-Rust `zbus`.

## [0.7.2] - 2024-08-05

## [0.7.1] - 2024-05-17

### Fixed

- On Linux, the `dbus` crate is now only pulled in when enabling the `reveal` feature (as was the case prior to `opener` v0.7.0).
- Fixed a Cargo error when compiling `opener` using versions of Rust prior to 1.71.

## [0.7.0] - 2024-03-22

### Added

- "dbus-vendored" feature, which is enabled by default to match current behavior. This just forwards to the `dbus` crate's "vendored" feature. Disable it to link dynamically to dbus instead of statically.

## [0.6.1] - 2023-04-14

## [0.6.0] - 2023-03-27

### Added

- `reveal()` function, which opens the system's file explorer with the specified file or directory selected. It requires the "reveal" feature to be enabled.

### Changed

- The error message when an executable is missing or otherwise fails to start is now more helpful due to the addition of the `OpenError::Spawn` variant, which is returned when spawning command(s) fails, and includes the name of the command(s). Before, these errors would be returned as `OpenError::Io`, which tend to be vague.
- `OpenError` is now marked `#[non_exhaustive]`.

### Fixed

- Path handling on Windows has been improved. `/` separators in relative paths are now accepted.
- Opening web links on WSL with `wslview` now works properly.

## [0.5.2] - 2023-01-29

### Fixed

- License files are now properly included in the published crate.

## [0.5.1] - 2023-01-28

### Changed

- Update `xdg-open`.

## [0.5.0] - 2021-06-11

### Added

- `open_browser()`, which uses the `$BROWSER` environment variable before falling back to `open()`.
- WSL-specific implementation. Previously, WSL used the same implementation as Linux. Now the strategy on WSL is to use the system's `wslview` command from [`wslu`](https://github.com/wslutilities/wslu) if available, falling back to the system `xdg-open`, if available.

### Changed

- On Linux (non-WSL), the system `xdg-open` is now used if present. Otherwise, the bundled version is used, as before.
- Avoid blocking the thread on Linux and WSL.

### Removed

- `impl From<io::Error> for OpenError`.

## [0.4.1] - 2019-09-30

### Changed

- Update `xdg-open`.

## [0.4.0] - 2019-05-02

### Added

- `OpenError` now implements `std::error::Error`.

### Changed

- `OpenError`'s `failure::Fail` impl was removed from this crate, but the failure crate provides a blanket impl of `failure::Fail` for types implementing `std::error::Error`, so this shouldn't break anything.

## [0.3.0] - 2018-08-18

### Added

- `stderr` field to `OpenError::ExitStatus` variant, which captures anything the failed process wrote to stderr.

## [0.2.0] - 2018-08-08

### Removed

- The `open_sys` function, which was erroneously pub on non-Windows builds.

## [0.1.0] - 2018-08-08

- Initial release.
