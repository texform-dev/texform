# Changelog

All notable changes to TeXForm are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). A single version number covers the Rust crate ([crates.io](https://crates.io/crates/texform)), the Python package ([PyPI](https://pypi.org/project/texform/)), and the JavaScript package ([npm](https://www.npmjs.com/package/texform)).## [0.4.0] - 2026-07-24

### Added

- **Breaking:** Default engine parsing to lenient

## [0.3.0] - 2026-07-21

This release adds a tokenized serialization channel, extends AST canonicalization with text-sequence normalization, and substantially expands the rewrite rule set — spacing, fraction and delimiter styling, negated-relation remaps, named-function operators, and more. It also renames the rule-level taxonomy on the public API and prunes several rules whose render fidelity did not hold up under corpus review.

### Added

- Tokenized serialization. `Document::to_tokenized_latex()` (and `to_tokenized_latex_with`) returns the canonical LaTeX string alongside ordered, typed output tokens — each classified as `ControlSequence`, `Character`, `Delimiter`, `Text`, `Raw`, or `Error`, and carrying its math/text mode and a non-overlapping UTF-8 byte span into that string. Tokens are recorded during the existing serializer traversal rather than by re-lexing the output, and the feature is exposed across the Rust, Python, and JavaScript APIs while leaving text-only serialization's allocation behavior unchanged.
- Text-sequence normalization in `FinalizeAst`. The profile-neutral canonicalization phase now merges adjacent text-mode siblings, collapses ordinary lexer whitespace runs to a single space without trimming edges, and cleans empty text. The pass re-runs after `FlattenGroups` so newly adjacent text and prime nodes are canonicalized, and its work is reported through the `normalize_text_sequences` step counter on the transform report.
- A large batch of rewrite rules across several normalization groups: spacing drops (`\enspace`, `\quad`, `\qquad`) and merges (adjacent `\enspace` pairs, small-spacer runs); fraction styling (`\dfrac`/`\tfrac`/`\cfrac` → `\frac`, `\dbinom`/`\tbinom` → `\binom`); limit placement (`\limits`/`\nolimits` drops on audited operators); fixed delimiter sizing (`\big`–`\Bigg` size drops); negated-relation remaps (`\not=` → `\neq`, `\not\exists` → `\nexists`, `\not\in` → `\notin`, `\not\rightarrow` → `\nrightarrow`); accent sizing (`\widehat` → `\hat`, `\widetilde` → `\tilde`, the first `Equiv`-level builtins); math-mode control-space to active space; duplicate `mathrel`-wrapper drops; and named-function rewrites for plain operator names, with extended `\operatorname` canonicalization. Shared delimiter-rewrite helpers back the delimiter-oriented rules.

### Changed

- **Breaking:** the rule-level taxonomy is renamed from *normalization level* to *rule level* across the public API, the transform engine, the generated registry, rule metadata, and documentation. The facade now re-exports `RuleLevelSet` in place of `NormalizationLevelSet`, and the internal `NormalizationLevel` enum becomes `RuleLevel`; downstream code referring to the old names must update. Active rules are reclassified under `Authoring`, `Faithful`, `Corpus`, and `Equiv` while preserving current profile behavior.
- `\limits`/`\nolimits` drops move from `Equiv` to `Corpus`, so they now participate in corpus normalization.

### Fixed

- Removed rewrite rules whose worst-case render fidelity did not hold across supported renderers: the `displaylines-to-gather-env` rule (its numbered `gather` output cannot preserve reading fidelity), the `repeat-spacer-collapse` rule, and the physics quick-`\quad` expansions (`qcomma-expand`, `qqtext-expand`). These forms are now preserved instead of rewritten.

## [0.2.1] - 2026-07-16

This release expands the normalization rule set with spacing canonicalization and a batch of alias and macro-expansion rules, so more spacing markup, legacy shorthand, and convenience commands collapse to canonical, universally renderable forms.

### Added

- Spacing normalization rules. Adjacent `\quad` pairs merge into `\qquad` and repeated small spacers collapse; MathJax spacing aliases — `\thinspace`, `\negthinspace`, `\hskip`, `\mkern`, `\mskip`, `\hfil`, `\hfilll`, `\space`, `\nobreakspace`, and `\gt` — map to their canonical spellings while preserving scalar dimensions and documented fidelity boundaries; and pure layout hints (`\mathstrut`, `\strut`, and line-break hints) are dropped.
- Alias and macro-expansion rules that fold convenience and legacy forms into universal output: `\ast` canonicalizes to the pixel-identical literal `*`; `\dots` resolves to `\ldots` or `\cdots` from the surrounding atom class, leaving unclassifiable and boundary cases untouched; `\impliedby` expands to the source-equivalent spaced `\Longleftarrow`; the `Vmatrix` environment expands into a matrix wrapped in explicit double-bar fences; and the `\bigl`/`\bigr`/`\bigm` class variants — with their `\Big`, `\bigg`, and `\Bigg` counterparts — collapse to the plain `\big`-family delimiters.

### Fixed

- Rule metadata is aligned with the published rule proposals, keeping each rule's declared normalization level and fidelity in sync with its documented contract.
- Rewrite rules that synthesize new commands — the implies, derivative, eval, and multi-integral expansions — now declare every character they emit, drawing on builtin character records so the engine's eliminated-form contract sees their full output.

## [0.2.0] - 2026-07-03

This release adds in-place document normalization to the Python and JavaScript bindings, teaches the parser and serializer to preserve whitespace and spacing faithfully, and makes parsing and transforming large formulas dramatically faster.

### Added

- `TransformEngine.transform` on the Python and JavaScript bindings, for normalizing a live `Document` in place. Every parsed document is stamped with a parse-context id, and a document produced by a different parser (or by `Document.from_syntax`) is rejected with a foreign-document error, so a document is only transformed by the engine that produced it.
- `:O` operator-name argspec content, so arguments to `\operatorname` and `\DeclareMathOperator` are modeled as math content and serialized compactly without special-casing command names.
- Typed serialization options for the Python bindings: `serialize()` and `Document.to_latex()` now accept a documented `SerializeOptions` TypedDict instead of an untyped `dict`, mirroring the TypeScript option interfaces.

### Changed

- **Breaking:** the public `Error` enum is now `#[non_exhaustive]`. Exhaustive `match` arms over `texform::Error` in downstream Rust must add a wildcard arm; in return, future error variants can be introduced without another breaking change.
- Parsing and transforming large formulas is dramatically faster. Release builds no longer run debug-only structural invariant sweeps, which were quadratic on wide formulas (a 60k-character formula now parses ~14× faster and transforms ~45× faster); source spans are carried as a positional tree (10–25% faster parsing); and rewrite rules are indexed by trigger name (31–48% faster transforms). Normalized output is byte-identical.

### Fixed

- Edge whitespace in text arguments is preserved: `\text{ or }` and `\textbf{ a }` no longer drop their leading and trailing spaces on parse, serialize, or transform.
- Adjacent math digits stay compact: multi-digit numbers such as `1093^2` no longer serialize as `1 0 9 3 ^ { 2 }` under the default spacing option, while letters and symbols still honor it.
- Tight argspec slot spacing is preserved, so no-leading-space slots — including linebreak dimensions and custom tight optional slots — stay tight through parse/serialize round trips.
- Whitespace is kept outside attribute wrappers during transforms.
- Inline math inside text-mode control sequences, and whitespace-only text arguments, are now accepted.
- The recovery parser is hardened against an unnecessary unwrap.
- The license file is included in the Python sdist.

## [0.1.0] - 2026-06-12

Initial public release of TeXForm — a LaTeX formula parser, editable document model, and normalization engine, available in Rust, Python, and JavaScript from a single Rust core.

### Added

- Knowledge-driven parser backed by 530+ command and environment specifications across the `base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, and `textmacros` packages, with strict and lenient modes that preserve unknown commands and unparseable fragments as explicit nodes instead of failing the parse.
- Editable `Document` tree with validated, fallible edits and canonical LaTeX serialization that guarantees text idempotency over parse/serialize cycles.
- Profile-based transform engine with four normalization profiles — `Authoring`, `Faithful`, `Corpus`, and `Equiv` — covering author-facing cleanup, render-faithful expansion, corpus preparation, and formula-equivalence comparison.
- `validate_argspec` for checking xparse-style argument specifications.
- Python (PyPI `texform`, Python ≥ 3.10) and JavaScript/TypeScript (npm `texform`, WebAssembly) bindings exposing the same parser, document, and transform engine from the shared Rust core.
