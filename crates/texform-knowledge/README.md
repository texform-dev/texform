# texform-knowledge

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate is TeXForm's command and environment knowledge base: which names are known, in which package, in which mode, and with what argument shape. At build time, `build.rs` compiles the YAML specifications in [`resources/specs/`](resources/specs/) into static Rust records, so lookups at runtime are allocation-free table reads.

Records cover commands, environments, characters, and delimiters across the built-in packages (`base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, `textmacros`, `color`). Argument shapes are expressed in the `texform-argspec` language and validated at compile time by the `argspec!` macro from `texform-knowledge-macros`.

LaTeX compatibility records describe parsing, not renderer support or macro expansion. Dimension arguments currently accept literal numeric lengths; expressions such as `-.5\height` in `\raisebox` remain unsupported even though they are valid LaTeX. Unknown macros and environments also cannot supply argument modes that are absent from the knowledge base, so a diagram inside a text container may need additional package definitions before it can be parsed.

The `color` package uses the LaTeX-compatible MathJax 3/4 declaration `\color[model]{color}` (`declarative`, `o:N m:N`), rather than the legacy two-argument `colorv2` macro. The text-mode `\textcolor` signature is supplied by `textmacros`, where MathJax delegates to the text parser (`o:N m:N m:T`); its math signature remains `o:N m:N m`. It also describes `\textcolor`, `\definecolor`, `\colorbox`, and `\fcolorbox`. Color names, model names, and specifications are string arguments; TeXForm preserves them without validating color models or expanding control sequences inside them. A local declaration needs its enclosing group, for example `{\color{red}x}+y`.
