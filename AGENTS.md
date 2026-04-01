# AI Agent Development Guide

## Project Overview

TeXForm is a **rapid-prototype project** for internal data processing.

### Language Conventions

**Important**: This project follows these language rules:

- **Source code** (`src/`): comments and identifiers MUST be in English
- **Documentation** (`docs/`): written in Chinese

## Core Principles

1. **Quick Fail**

- Fail fast on invalid input — no complex recovery
- Use `panic!` or `unwrap()` freely for programming errors
- Reserve `Result<T>` for expected user-facing errors only
- No large-scale fault tolerance or boundary fallbacks

2. **Rapid Iteration**

- Deliver core functionality first
- Minimize abstractions — prefer concrete implementations
- Prefer single-file modules
- No premature optimization
- No external users yet — backward compatibility not required

3. **Code Quality**

- **Concise and precise**: clear expression, minimal boilerplate
- **Comment policy**:
  - Code itself should express *what* through naming — do NOT restate the code in comments
  - Comments exist to explain **why**: design decisions, non-obvious constraints, rejected alternatives, correctness arguments
  - Worth a comment: why `debug_assert!` instead of returning an error? Why is this field ignored? Why does ordering matter here? What invariant is being maintained?
  - Doc comments should describe the function's responsibility and key semantics (e.g. any-match vs all-match) — do not repeat what the signature already says
  - Do not explain the same concept multiple times within one function — say it once, in the best place
  - `unreachable!()` and `debug_assert!()` messages should help a debugger understand *why it should never fire*, not restate the surrounding code
  - **Writing no comments is just as harmful as writing bad comments** — rapid prototyping does not mean unreadable code
- **English only in source**: all code comments, identifiers, and inline documentation must be in English

4. **Testing Strategy**

- Place public-API tests in `tests/` (e.g. `tests/ast.rs`); place internal-implementation tests inline in modules
- Tests should serve as executable documentation
- Cover the happy path + 2–3 key edge cases
- No exhaustive testing — prioritize fast validation

## Code Style

- Comments and identifiers under `crates/` are always in English
- Prefer `unwrap()` in internal code — avoid verbose error handling
- Use `todo!()` liberally for unimplemented branches, with a brief English note

## Available CLI Examples

Two CLI examples (`parse` and `validate_spec`) are available for quick inspection and debugging. 