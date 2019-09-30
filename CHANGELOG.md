# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
