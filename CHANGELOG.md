# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- `open_browser()`, which uses the `$BROWSER` environment variable before falling back to `open()`.
- WSL-specific implementation. Previously, WSL used the same implementation as Linux. Now, the strategy is to use the
system `xdg-open` if available, otherwise we try using the system's `wslview` command from
[`wslu`](https://github.com/wslutilities/wslu).
### Changed
- On Linux (non-WSL), the system `xdg-open` is now used if present. Otherwise, the bundled version is used, as before.
- The command name in the `OpenError::ExitStatus` variant is now returned as a `Cow<'static, str>` instead of a
`&'static str`.
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
