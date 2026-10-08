# Changelog

All notable changes to TeXForm are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). A single version number covers the Rust crate ([crates.io](https://crates.io/crates/texform)), the Python package ([PyPI](https://pypi.org/project/texform/)), and the JavaScript package ([npm](https://www.npmjs.com/package/texform)).
## [0.9.1] - 2026-10-08

### Added

- Add the `color` package
- Add unknown input warnings to transform reports
- Add `list_rules` rule catalog

### Fixed

- Preserve unknown command argument groups

## [0.9.0] - 2026-10-04

This release is a round of fixes that make TeXForm safer to run on real-world input. Parsing no longer panics on any source text, reports each problem with its specific diagnostic kind at its actual source position, and reads several plain TeX constructs as TeX does instead of silently misparsing them. Input that TeXForm cannot read correctly is now rejected with a diagnostic rather than turned into a wrong tree. The `Corpus` profile no longer drops modifiers whose visible effect depends on the math style, so its output keeps the rendering of its input. To parse the plain TeX macros `\root…\of` and `\buildrel…\over` correctly, the release adds the `u{\name}` form to the ArgSpec syntax; its new argument-form variant is the only breaking change, for Rust code that matches argument forms exhaustively.

### Added

- **Breaking:** The ArgSpec syntax gains the until form `u{\name}`. It describes plain TeX delimited parameters such as `\def\root#1\of#2`, whose argument runs up to a terminating control word instead of being one token or one brace group, which no existing form could express. The argument collects tokens up to the first terminator outside brace groups and strips one brace layer when it is exactly one group; empty content is valid, and a missing terminator is a parse error. The terminator must be a single control word such as `\of` or `\over`, and ArgSpec validation, including for custom entries, rejects characters, control symbols, and multi-token terminators. `ArgumentKind`, `ArgKindRef`, and `ArgSpecFormInfo` gain an `Until { close }` variant, and bindings, columnar export, and CLI output name the form `"until"`; exhaustive matches over argument forms must handle it.
- The knowledge base covers LaTeX text boxes and related commands, so input such as `\text{\mbox{$x$}}`, `\raisebox{1ex}{$x$}`, or `\left\llbracket x\right\rrbracket` parses. `\raisebox` and `\scalebox` are recognized with their optional arguments; `\mbox`, `\fbox`, `\hbox`, `\makebox`, and `\parbox` work in text mode as well as math mode; `\label` and the `math` environment are allowed inside text containers; `\ensuremath` reads its argument as math in both modes; nested `\text` is allowed in text mode; `\emph` works in math mode; and `\llbracket` and `\rrbracket` are delimiters. These records describe parsing, not renderer support, and dimension arguments still require literal lengths.

### Changed

- The ArgSpec of the command `root` changes from `m` to `u{\of} m`. With `m`, the parser read only one token or group as the degree and left `\of` and the radicand as sibling nodes, so unbraced degrees such as `\root n_i\of{x}` were misparsed and `\root\of{x}` was rejected. A parsed `root` now has a degree slot and a radicand slot, and a `\root` without `\of` is a parse error. Normalized output is unchanged for previously accepted input.
- The ArgSpec of the command `buildrel` changes from `m` to `u{\over} m`. With `m`, `\buildrel` took only the first token of the annotation, and the parser used a source-text lookbehind to split the rest with an infix `\over`, so `a\over\buildrel b\over c` was rejected as an ambiguous infix and a braced `\over` inside the annotation was mistaken for the separator. A parsed `buildrel` now has an annotation slot that runs to the first `\over` outside braces and an operator slot, and a `\buildrel` without `\over` is a parse error. Scripts after the operator attach to the whole relation. The Faithful rule `base/buildrel-expand` is replaced by the Authoring rule `base/buildrel-to-stackrel`, which rewrites `\buildrel A\over B` to `\stackrel{A}{B}`; rule selections that name the old key should use the new one. Faithful, Corpus, and Equiv output is unchanged for previously accepted input, and Authoring now succeeds instead of failing its rewrite contract.
- `Corpus` and lower profiles keep `\dfrac`, `\tfrac`, `\limits`, and `\nolimits`. Their visible effect depends on the math style, which a single formula does not know, so dropping them could change the fraction size or move the scripts. `Equiv` still rewrites them.

### Fixed

- Source text containing a character the lexer cannot tokenize, such as an ASCII control character other than tab, newline, carriage return, or form feed, returns an `invalid-character` diagnostic instead of panicking. In Python the panic surfaced as `PanicException`, which escaped ordinary `except Exception` handlers; callers now receive a regular `ParseError`.
- Diagnostics name the specific problem. Internal parser branch failures such as `not a command` no longer replace the real diagnostic, an environment left open at end of input reports `unclosed-environment` with `\end{name}` as the expected token, and an environment used in a disallowed mode reports `environment-mode-error` in both strict and lenient parsing. `ParseDiagnosticKind` in Python and TypeScript includes the new kinds.
- Diagnostics point at the actual source position. With `reject_unknown`, `\frac{\bad}{x}` reported the unknown command at `\frac`, and `a+\bad` at `a`; spans inside argument content were also shifted after a comment or a run of spaces. Errors now keep the failing token's span, and diagnostics and node spans inside arguments map token by token back to the source.
- Plain TeX alignment macros (`\matrix`, `\pmatrix`, `\cases`, `\eqalign`, `\eqalignno`) split rows at `\\` and `\newline` as well as `\cr`, and drop a final row terminator that has no `*` or spacing. `\eqalignno` no longer fails unless every row ends in `&(…)`: it becomes `align*`, and only rows with a numbering cell get a `\tag` (or `\tag*` when the cell is not wrapped in parentheses).
- A script at the start of an alignment row attaches to an empty base in that row instead of to the preceding row separator: `\begin{matrix}a\\_i b\end{matrix}` no longer serializes as `\\ _{i} b`.
- In `array`, `darray`, `eqnarray`, and `eqnarray*`, `\\ [-5pt]` reads the bracket as row spacing, as LaTeX does. amsmath environments still read it as row content, so a row can start with an interval such as `[a,b]`.
- An unbraced `\left…\right` or `$…$` argument keeps its delimiters: `\frac\left(x\right)2` parses like `\frac{\left(x\right)}{2}` instead of `\frac{x}{2}`, and `\text$x$` like `\text{$x$}`.
- A comment between a control word and following letters inside an argument no longer merges them: `\sqrt[\le% comment` followed by `q]{x}` on the next line was read as `\sqrt[\leq]{x}`.
- Custom signatures with delimiters such as `d**` or `r~~` match `*`, `$`, `&`, `#`, `^`, `_`, and `~` in the input instead of always treating the argument as absent. ArgSpec validation rejects delimiters that can never match: `%`, `'`, `’`, and ASCII control characters.
- `\hspace*{…}` parses in math and text modes.
- A text environment whose body ends with a group, inline math, or whitespace before `\end` no longer reports a spurious diagnostic.

## [0.8.0] - 2026-09-30

This release makes `Document` a complete API for building and editing formulas. Every node kind now has a constructor, so a formula can be built from scratch as a tree and serialized, and every construction and edit is checked against the document's knowledge base, so the result always conforms to it, as parser output does, and transforms accept it. Deterministic edits, rooted node paths, cross-document imports, and a columnar export cover targeted edits and bulk analysis. To support this, `KnowledgeBase` becomes an immutable object shared by parsers, engines, and documents. The release also fixes script, text-argument, and declaration-scope handling in alignments and text mode. The knowledge base and construction changes are breaking; see **Changed** for migration notes.

### Changed

- **Breaking:** Package selection, custom items, and removals move from parser and engine builders to `KnowledgeBaseBuilder`, and knowledge queries move from `Parser` and `TransformEngine` to `KnowledgeBase`. Build one `KnowledgeBase` and pass it to every parser, engine, and document that should interoperate, for example `Parser::builder().knowledge_base(kb.clone()).build()` and `TransformEngine::builder().knowledge_base(kb)`. `ParserBuilder::build()` now returns `Parser`, and `ParserBuildError` and `Error::ParserBuild` are removed. `TransformEngine::transform` accepts any document bound to the same instance and returns `Error::KnowledgeBaseMismatch` otherwise, replacing `Error::ForeignDocument`. In Python, pass `texform.KnowledgeBase(packages, items=...)` to `Parser(kb)` and `TransformEngine(profile, kb)`; in JavaScript, pass `{ knowledgeBase }`. The command-line tool and the `serve` protocol are unchanged.
- **Breaking:** Construction, edits, and syntax imports keep every complete document conformant to its knowledge base. A violation returns `ConformanceError` with a rooted path and a stable rule code (`TexformConformanceError` in JavaScript) and leaves the document unchanged. Replace `ArgValue` with `Arg`, and binding argument dictionaries with `Node | str | bool | None | Paired` values (`{ value, open, close }` pairs in JavaScript). `create_group` and `create_environment` take their children and body directly, binding delimiters are strings such as `"("` and `"\\langle"`, and environments are renamed with `set_env_name` instead of `set_command_name`. `Document::from_syntax` rejects non-conformant complete trees, with `FromSyntaxError::Conformance` replacing the prime-specific variants, and `KnowledgeBaseBuilder` rejects item names the lexer cannot produce with `KnowledgeBaseBuildError::InvalidName`.

### Added

- Every node kind has a constructor, including scripted nodes, infix commands, primes, inline math, and delimited groups. Commands and environments are built from their knowledge-base records, so optional, star, and paired slots can be filled, and a constructor called with a name of another command kind names the one to use. Arguments are given as nodes, source text parsed in the slot's mode, star Booleans, or explicit absent and paired values; `in_mode` and `parse_fragment` construct in an explicit context mode.
- Deterministic edits: `set_subscript` and `set_superscript` add, change, or clear scripts, collapsing the wrapper when the last script is cleared; `set_delimiters`, `set_arg_delimiters`, and `set_prime_count` change delimiters and prime counts. `clone_node` copies a subtree within a document, and `import_node` copies one from another document, validating it against the destination knowledge base. Failed edits leave the document unchanged.
- `NodeRef::path` returns a node's rooted path and `Document::node_at` resolves it; `NodeRef::slot` and `NodeRef::is_known` report the parent slot and knowledge status. Python and JavaScript expose the same edits and reads, JavaScript adds `argKind(index)`, and argument reads include their slot form.
- `Document::to_columnar` exports a document as a preorder node table and an argument table for Arrow and DataFrame tools (`to_columnar()` in Python, `toColumnar()` in JavaScript); Python tables pass directly to `pyarrow.table`.
- `KnowledgeBase` instances are cheap to clone and compare by identity (`KnowledgeBase::ptr_eq`); `KnowledgeBase::default()` returns one process-wide instance, and `Document::with_knowledge_base` and `Document::from_syntax_with` bind an explicit one. Python and JavaScript expose `KnowledgeBase` with identity equality and hashing, document copies (`copy()` in Python, `clone()` in JavaScript), node identity, and `document()` on nodes.

### Fixed

- In alignments, a script directly after a column separator (`&^2`, `&_1`, `&'`) now starts the next cell with an empty base instead of using `&` as its base. Escaped `\&` still takes scripts.
- An unbraced argument of a text-mode command takes a single token, as in TeX and MathJax: `\mbox TeV` parses as `\mbox{T}eV` instead of `\mbox{TeV}`. `\text{$x$ }` and `\textbf{ \emph k }` now parse.
- Alignment separators (`&`, `\\`, `\newline`, `\cr`) end declarative scopes in environment bodies and math arguments, so `\begin{array}{cc} \rm a & b \\ c & d \end{array}` lowers to `\mathrm{a} & b \\ c & d` instead of the invalid `\mathrm{a & b \\ c & d}`, and a `\displaystyle` repeated in a later cell is kept.

## [0.7.0] - 2026-09-26

This release makes normalized output reparse to the same tree, so normalizing it again no longer changes it, and stops math-mode `\&` from turning into an alignment tab. Both fixes change the tree shape; see **Changed** for migration notes.

### Changed

- **Breaking:** A bare `&` now parses as a new `AlignmentTab` node in `SyntaxNode`, `Node`, and `NodeKind`, and `Char('&')` always means a literal ampersand. Code that matched or built `Char('&')` for an alignment tab should use `AlignmentTab`, and exhaustive matches need the new variant. `Document::create_alignment_tab` stages one (`create_alignment_tab` in Python, `createAlignmentTab` in JavaScript); node kinds report `"AlignmentTab"` in Python and `"alignmentTab"` in JavaScript.
- **Breaking:** A command argument whose content is a single brace group now parses to an implicit container around that group, so a brace group directly in any slot owns that slot's braces. FlattenGroups always unwraps redundant user braces in argument slots, and the `command_argument` guard and its report field are removed from Rust, Python, and JavaScript.
- The default packages are now `base`, `ams`, `braket`, `textmacros`, `bboldx`, and `boldsymbol`; `physics` must be loaded explicitly.

### Fixed

- Normalizing the output of `normalize` again no longer changes it. Argument braces around a single brace group are kept (`\overline{{{\Psi}}}`, `\mathrm{{}}`), braces that protect a closing delimiter inside an optional argument are restored on output (`\xrightarrow[{]}]{x}` no longer becomes `\xrightarrow [ ] ] { x }`), and a lone `\` at a line end or input end is a control space, as in TeX.
- Optional, `Delimited`, and `Paired` arguments track brace depth, so protected content such as `\qty[{]}]` parses.
- In text mode, whitespace after an argument-free control word is no longer content: `\text{\bf\large x}` no longer normalizes to `\textbf{ x}`.
- Math-mode `\&`, such as `a \& b` or `\mathrm{\&}`, is serialized as `\&` instead of a bare `&` that became a column separator inside `matrix` or `align`. All profiles were affected, including plain parse and serialize.

## [0.6.0] - 2026-09-25

This release adds the `texform` command-line tool, gives prime marks a consistent meaning as symbols with explicit superscript binding, and makes normalization stable across transform phases. It also fixes spacing drift after text-mode control symbols and declarations leaking out of local groups, and roughly halves transform time. The prime change is breaking; see **Changed** for migration notes.

### Added

- A new `texform-cli` crate installs a `texform` binary (`cargo install texform-cli`). `normalize`, `parse`, and `tokenize` read one formula from an argument or stdin, or one per line with `--lines`, and print human-readable text or one JSON object per formula; `info`, `packages`, and `argspec validate` query the knowledge base and argument specifications with the same shapes as the bindings. `texform serve` speaks JSON-RPC 2.0 over stdio (normalizer protocol v1) so external tools can call a specific TeXForm build across a process boundary, and `texform --version` reports the source commit and whether it was dirty. The CLI loads the library's default packages; `braket` is opt-in.

### Changed

- **Breaking:** `Prime` is now a math symbol, and only `Scripted` expresses superscript binding. `x\prime` stays `x \prime` instead of becoming `x'`, `x^2\prime` no longer produces an unparsable double superscript, `f'^2` serializes as one superscript instead of a nested empty-base one, and `f^{'}` no longer collapses to `f'`. A pure prime superscript still serializes as `f'`. A leading quote, including inside `f^{'}` and `A^{'\alpha}`, parses as an empty-base `Scripted`, and whitespace-separated quotes such as `f' ' '` form one superscript as in MathJax. `x^'` and `x_'` are now parse errors. Snapshots that stored a bare `Prime` for a quote superscript should be reparsed from source or rebuilt with an empty-base `Scripted`.
- Transforms are faster: rewrite trigger indexes are built once per `Plan`, and phases skip work they have already done. On a 10,000-formula sample, transform time falls from 33–44 µs to 14–18 µs per formula across the four profiles.

### Fixed

- Normalization no longer depends on how many times it is applied. Later phases could expose attribute merges or rewrite matches after earlier phases had finished; the phases now run to a shared fixed point, and a transform that fails to converge within eight rounds returns a `TransformError` instead of partially normalized output. As a result, a prefix command's single-node argument is no longer wrapped in an implicit group in the transformed tree; serialized output is unchanged.
- Text-mode control symbols such as `\ ` and `\,` no longer gain a separator space on every serialization (`\mbox{mod\ 1}` grew by one space per round). A separator is emitted only where it keeps a control word from absorbing the following letter.
- Declaratives without a prefix form keep their local scope: `{\scriptstyle x} y` no longer becomes `\scriptstyle x y`, `{\oldstyle 1}2` no longer becomes `\oldstyle 12`, and `\mathbf{\scriptstyle x}y` no longer emits the style twice.

## [0.5.0] - 2026-09-22

This release reshapes the configuration and reporting surface across Rust, Python, and JavaScript so the three APIs share one configuration tree and one set of override semantics. Transform reports become opt-in, `TransformConfig` is organized by phase, serialization options are flattened, and binding exception categories are corrected. It also hardens parser recovery and diagnostics, fixes serialization of bare delimiter arguments, and adds two corpus rules. Most changes are breaking; see **Changed** for migration notes.

### Added

- `normalize_with_report` / `transform_with_report` on the Rust facade and the Python binding (`normalizeWithReport` / `transformWithReport` in JavaScript) for callers that need the diagnostic report. Report types now live in `texform::diagnostics`.
- Effective defaults are exposed on every binding: `default_parse_config()` / `default_transform_config()` in Python and `defaultParseConfig()` / `defaultTransformConfig()` in JavaScript return the engine's profile defaults. Rust adds `TransformEngine::default_normalize_config()`.
- `RewriteConfig`, plus the serialization option enums (`CommandSpacing`, `MathGroupInnerSpacing`, `AdjacentCharSpacing`, `ScriptSpacing`, `ScriptOrder`, `InfixGrouping`, `EnvironmentNameSpacing`), are re-exported from the Rust facade.
- Corpus rules `leftroot-drop` and `uproot-drop` drop the AMS `\leftroot` / `\uproot` index-position hints.

### Changed

- **Breaking:** transform reports are opt-in. `normalize` / `normalize_with` now return the canonical string and `transform` / `transform_with` return nothing, in Rust, Python, and JavaScript, so plain calls no longer pay for report collection or host-object conversion. `NormalizeResult` and the `FinalizeAstStep*` types are removed. The report itself is restructured (`rewrite.{iterations, rules}`, flattened FinalizeAst counters, `flatten_groups.guard_hits` with short guard names) and remains a diagnostic rather than a stability promise.
- **Breaking:** `TransformConfig` is organized by phase. The flat `rewrite_enabled`, `lower_attributes_enabled`, and `max_iterations` fields become `rewrite: RewriteConfig { enabled, max_iterations }` and `lower_attributes: LowerAttributesConfig`, alongside the existing `finalize_ast` and `flatten_groups`. Profile defaults are unchanged.
- **Breaking:** `FlattenGroupsConfig` exposes only `enabled` and `preserve_rendered_spacing` (`preserveRenderedSpacing` in JavaScript). The eleven `preserve_*` guard fields are removed and old keys are rejected.
- **Breaking:** serialization options are a flat set of seven fields; the nested `math` / `syntax` shape is removed. Python takes snake_case keyword arguments (`doc.to_latex(script_order="sup_first")`, `serialize(node, **options)`), and JavaScript takes a flat camelCase object.
- **Breaking:** configuration inputs are unified across bindings. Python reserves `config=` for a complete config object (`TransformConfig`, `ParseConfig`); partial settings are passed as keyword overrides that keep unspecified defaults, and a dict passed as `config` is rejected. JavaScript accepts the same nested override tree in `normalize` and `transform`, with omitted fields keeping the profile default. Unknown keys and wrongly typed values raise `ConfigError` / `TexformConfigError` with the field path. The unused WASM config classes and `parseWith` are removed.
- **Breaking:** binding exception categories are corrected. Python cross-document edits raise `EditError` instead of `ParseError`. Transforming a document that has parse errors raises `TransformError` in Python and `TexformTransformError` (`kind: "transform"`) in JavaScript instead of the `internal` kind. The undocumented `Complete<T>` type is no longer importable from the npm typings.

### Fixed

- Parser recovery under `reject_unknown` without `abort_on_error` consumes only the unknown command, so `\mycmd{x} + y` yields a partial document instead of failing the whole parse. A stray `}` is no longer reported as an environment-name mismatch, and `^` / `_` with missing content report `Missing superscript content` / `Missing subscript content`.
- Diagnostics keep their structured kind and precise source span through nested content parsing instead of highlighting the outer command or closing brace, and internal kind labels no longer leak into messages.
- Literal carriage returns are treated as whitespace, so Windows (CRLF) and CR-only inputs parse instead of panicking.
- Mandatory delimiter arguments are serialized without braces (`\middle \vert`, `\big \langle`, `\genfrac . . {} {}`), so the output renders in MathJax again.
- JavaScript: `normalize` with a `flattenGroups` override now layers it on the engine profile's defaults instead of the strict baseline.
- JavaScript: internal DTO serialization failures are thrown as `TexformError` (kind `internal`) instead of being returned as successful values.
- Python: invalid per-call config objects raise `ConfigError` consistently across `parse`, `normalize`, and `transform`.

## [0.4.0] - 2026-07-26

This release aligns `TransformEngine` parsing with the standalone `Parser` by defaulting to lenient parsing, and fixes prime superscripts inside environment bodies.

### Changed

- **Breaking:** `TransformEngine` now parses with `ParseConfig::LENIENT` by default (previously `ParseConfig::STRICT`), matching the standalone `Parser`; this applies to Rust, Python, and JavaScript. Unknown commands are preserved instead of rejected, so `normalize(r"\unknowncmd")` now succeeds, and `engine.parse` returns a recovery document with error nodes for malformed input. `normalize` still requires a complete tree and raises a parse error, carrying the diagnostics and the partial document, when the input cannot produce one. To restore the previous behavior, pass `ParseConfig::STRICT` as the engine's default parse config, or set `reject_unknown` and `abort_on_error` to `true` (`rejectUnknown` / `abortOnError` in JavaScript).

### Fixed

- A lone prime inside an environment body, such as an `array` cell, no longer serializes as an explicit superscript group that renders as a double superscript. FlattenGroups now unwraps lone `Prime` nodes in superscript slots while still preserving other environment-body groups.

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
