# texform-argspec

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate parses TeXForm's xparse-style argument-specification language — the compact signatures (mandatory `m`, optional `o`, star `s`, delimited `d`, and friends) that describe how each LaTeX command and environment consumes its arguments.

The required until form `u{\name}` collects an argument through one control-word terminator, ignoring occurrences inside brace groups, and strips one enclosing brace layer when the argument is a single group. For example, `u{\of} m` describes the degree and radicand of `\root n_i\of{x}`. Empty content is valid; a missing terminator is an error. The terminator must be exactly one control word such as `\of` or `\over`; characters, control symbols, and multi-token terminators are rejected.

It is consumed by `texform-knowledge` (every knowledge-base record carries an argspec), by the parser in `texform-core` (to drive argument consumption), and by the public `validate_argspec` API on the facade.
