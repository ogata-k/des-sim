# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-20

Initial release.

### Added

- **Context / Execution / Runner**: Implemented an execution environment for discrete event simulation using ticks and
  micro-steps.
- **Modeling**: Implemented a modeling foundation to define simulation targets and perform event-driven processing.
- **Source**: Added functionality to manage and generate periodic event schedules.
- **Hooks**: Implemented standard hooks to support log output, step execution (debugging), and state extraction during
  simulation execution.
- **Sampler**: Added various functions and combinators to support statistical sampling (random numbers).
- **Features**:
  - `verbose_debug`: Enable detailed log output.
  - `des_sim_test_mode`: Optimize behavior during test execution.
- **Documentation**: Prepared standard documentation in English (README.md) and Japanese (README-ja.md).