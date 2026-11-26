# texform-argspec

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate parses TeXForm's xparse-style argument-specification language — the compact signatures (mandatory `m`, optional `o`, star `s`, delimited `d`, and friends) that describe how each LaTeX command and environment consumes its arguments.

The required until form `u{\name}` collects an argument through one control-word terminator, ignoring occurrences inside brace groups, and strips one enclosing brace layer when the argument is a single group. For example, `u{\of} m` describes the degree and radicand of `\root n_i\of{x}`. Empty content is valid; a missing terminator is an error. The terminator must be exactly one control word such as `\of` or `\over`; characters, control symbols, and multi-token terminators are rejected.

A slot may carry a value-kind annotation after `:`; without one it holds math content. A trailing `?` (for example `m:R?`) marks the slot nullable: an empty argument such as `{}` is accepted where the value kind would otherwise reject it. Star and column slots cannot be nullable.

| Annotation | Value kind | Example use |
| --- | --- | --- |
| `:T` | Text-mode content | `\text{...}` |
| `:O` | Operator-name content, parsed as math | `\operatorname{...}` |
| `:D` | One delimiter | `\middle\vert` |
| `:N` | A literal string without control sequences: kept as text rather than parsed, with whitespace runs collapsed | label keys, color names and models, position letters such as `[t]` |
| `:CS` | Exactly one control sequence, bare or braced (`\abs` or `{\abs}`), stored with its backslash | names defined by macro-definition commands, such as `\abs` in `\DeclarePairedDelimiter\abs{\lvert}{\rvert}` |
| `:L` | A dimension with a unit | `\hspace{1em}` |
| `:I` | An integer | `\romannumeral 12` |
| `:R` | A unitless decimal (`[+-]?(\d+(\.\d*)?\|\.\d+)`) | scale factors and other unitless amounts, such as `1.5` |
| `:K` | A `key=value` list | option lists |
| `:C` | A column template | `\begin{array}{cc}` |

Scalar kinds (everything except content and operator-name slots) are stored as strings and are never parsed as LaTeX content: a `:CS` value is not looked up as a command. `:C` and `:CS` are distinct annotations; `:L` requires a unit, `:I` rejects decimals, and `:R` rejects units and decimal commas.

It is consumed by `texform-knowledge` (every knowledge-base record carries an argspec), by the parser in `texform-core` (to drive argument consumption), and by the public `validate_argspec` API on the facade.
