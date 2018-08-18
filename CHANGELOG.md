# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2018-08-18
### Added
- `stderr` field to `OpenError::ExitStatus` variant, which captures anything the failed process wrote to stderr.

## [0.2.0] - 2018-08-08
### Removed
- The `open_sys` function, which was erroneously pub on non-Windows builds.

## [0.1.0] - 2018-08-08
- Initial release.
