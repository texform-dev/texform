# texform-argspec

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate parses TeXForm's xparse-style argument-specification language — the compact signatures (mandatory `m`, optional `o`, star `s`, delimited `d`, and friends) that describe how each LaTeX command and environment consumes its arguments.

It is consumed by `texform-knowledge` (every knowledge-base record carries an argspec), by the parser in `texform-core` (to drive argument consumption), and by the public `validate_argspec` API on the facade.
