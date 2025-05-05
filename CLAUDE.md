# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Approach
- Use Test-Driven Development (TDD) for implementing XAFS functions:
  1. Generate reference data with xraylarch Python script
  2. Write test using reference data
  3. Implement function in Rust to match reference output
  4. Add parallel (`_par`) implementation for performance
- Reference implementation: xraylarch (Python)
- Goal: Implement xraylarch XAFS functions in Rust with improved performance
- Compare outputs with xraylarch for validation (test files in tests/testfiles/)
- Current test status: 56/57 tests passing, 1 test ignored (test_xas_jsongz_read)

## Build/Test Commands
- Build: `cargo build`
- Format: `cargo fmt`
- Run tests: `cargo test`
- Run single test: `cargo test test_name`
- Run specific tests: `cargo test normalize` (runs all tests with "normalize" in name)
- Run test with debug output: `cargo test test_name -- --nocapture`
- Fix minor issues: `cargo fix --lib -p xraytsubaki --tests` (applies automatic fixes)
- Benchmarks: `cargo bench`
- Run specific benchmark: `cargo bench --bench xas_group_benchmark_parallel`

## Python Tools
- Use `uv` for Python dependency management
- Install packages: `uv add package_name`
- Install test requirements: `cd crates/xraytsubaki && uv pip install -r tests/pythonscript/requirements.txt`
- Run Python scripts: `uv run python script.py`
- Generate test data: `cd crates/xraytsubaki && uv run python tests/pythonscript/generate_test.py`

## Code Style Guidelines
- Follow Rust's standard conventions - use `rustfmt` for formatting
- Use snake_case for variable/function names, CamelCase for types/traits
- Error handling: Use `thiserror` for library errors, `anyhow` for application code
- Define custom error types with `#[derive(Debug, Error)]` and use `#[error("message")]`
- Module structure: group related functionality in modules
- Use `ndarray` for numerical work and follow its conventions
- Tests should be organized by module, use appropriate tolerance constants
- Constants naming: `UPPER_SNAKE_CASE`
- Prefer parallel implementations where possible (with `_par` suffix)
- Documentation: Use doc comments `///` for public items