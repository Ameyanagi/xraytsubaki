# Modernize Error Handling with thiserror

## Summary
Replace manual `Error` trait implementations with `thiserror`-based error types following Rust 2024 best practices, eliminating deprecated methods, reducing boilerplate, and improving error ergonomics for library consumers.

## Motivation
The current error handling implementation has several issues:

1. **Uses deprecated Error methods**: `description()` and `cause()` are deprecated in favor of `Display` and `source()`
2. **High boilerplate**: Manual `Error` and `Display` trait implementations are repetitive and error-prone
3. **Limited context**: `Box<dyn Error>` loses type information that callers might need
4. **Fragile error propagation**: 227 instances of `.unwrap()`/`.expect()` and several `panic!()`/`todo!()` calls
5. **No automatic conversions**: Missing `From` implementations force manual error mapping

## Goals
- ✅ Modernize error types using `thiserror` derive macros
- ✅ Eliminate deprecated Error trait methods
- ✅ Add context-rich error messages with location information
- ✅ Create domain-specific error enums for different modules
- ✅ Enable automatic error conversion via `#[from]` attribute
- ✅ Replace panic/todo with proper error returns where feasible
- ✅ Maintain zero-cost abstractions (thiserror has no runtime overhead)

## Non-Goals
- ❌ Adding `anyhow` (this is a library, not an application)
- ❌ Adding backtrace support (performance overhead for scientific computing)
- ❌ Breaking API changes to public error types
- ❌ Replacing all `.unwrap()` calls (some are legitimate in tests/benchmarks)

## Scope
This change affects:
- `crates/xraytsubaki/src/xafs/mod.rs` - Core `XAFSError` type
- `crates/xraytsubaki/src/xafs/*.rs` - Module-specific error types
- `Cargo.toml` - Add thiserror dependency
- Error propagation patterns across the codebase

## Impact Assessment
- **Breaking Changes**: None if we maintain existing error variant names
- **Performance**: Zero runtime impact (thiserror is compile-time only)
- **Dependencies**: +1 dependency (`thiserror = "2.0"`)
- **Migration Effort**: Medium (72 `Box<dyn Error>` signatures, ~20 error sites)

## Alternatives Considered
1. **Keep manual implementations**: Rejected due to deprecation warnings and maintenance burden
2. **Use `anyhow`**: Rejected - inappropriate for libraries (loses type information)
3. **Use `snafu`**: Over-engineered for this use case; thiserror is simpler and more widely adopted
4. **Custom error framework**: Rejected - reinventing the wheel

## Dependencies
- Requires: None
- Blocks: None
- Related: Future work on Result type aliases and error recovery strategies
