# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
## [0.0.6](https://github.com/asincole/docuparse/compare/v0.0.5...v0.0.6) - 2026-10-01

### Other

- *(release)* re-enable semver checks
- *(release)* move release-plz config to release-plz.toml

## [0.0.5](https://github.com/asincole/docuparse/compare/docuparse-v0.0.4...docuparse-v0.0.5) - 2026-09-30

### Other

- *(release-cli)* collect release assets without unmatched-glob failures
- switch caching from mr-boxington to Swatinem rust-cache
- *(release-cli)* drop the mac intel target
- *(release-cli)* drop the Windows target until ort-sys supports cross-target builds
- *(release-cli)* bypass the mbx cargo shim on Windows
- *(release)* align tag naming, changelog entries, and binstall metadata
- release ([#10](https://github.com/asincole/docuparse/pull/10))

## [0.0.4](https://github.com/asincole/docuparse/compare/v0.0.3...v0.0.4) - 2026-09-30

### Added

- *(cli)* add docuparse-cli crate with release workflow ([#9](https://github.com/asincole/docuparse/pull/9))

### Other

- release v0.0.4
- *(deps)* refresh dependencies to latest ([#8](https://github.com/asincole/docuparse/pull/8))

## [0.0.3](https://github.com/asincole/docuparse/compare/v0.0.2...v0.0.3) - 2026-05-21

### Added

- pluggable OCR backends with ONNX and OpenAI-compatible vision API ([#3](https://github.com/asincole/docuparse/pull/3))

## [0.0.2](https://github.com/asincole/docuparse/compare/v0.0.1...v0.0.2) - 2026-04-17

### Added

- initial docuparse library

### Other

- automate releases with release-plz and update readme
- initial commit
