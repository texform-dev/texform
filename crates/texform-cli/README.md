# texform-cli

Command-line interface for [TeXForm](../../README.md), installed as the `texform` binary. It exposes the public `texform` facade to shell users and to external tools that call TeXForm across a process boundary.

## Installation

```bash
cargo install texform-cli
```

To build from a source checkout, run `cargo install --path crates/texform-cli` from the repository root. Either way the installed binary is named `texform`.

## Build identity

`texform --version` identifies the source tree a binary was built from:

```text
texform 0.5.0 (0123456789ab 2026-09-22, dirty)
```

- The version is the crate version.
- The parenthesized part is the abbreviated commit hash and its committer date (`YYYY-MM-DD`). It is omitted when the build had no git information, for example when installed from crates.io.
- `dirty` means the build inputs differed from that commit: tracked changes or untracked files under `crates/`, `Cargo.toml`, or `Cargo.lock`. Changes elsewhere, such as documentation, do not make a build dirty.

The serve protocol reports the same identity in `serverInfo`.

## Usage

```text
texform [--packages <a,b,...>] <COMMAND> [OPTIONS]
```

| Command | Purpose |
| --- | --- |
| [`normalize`](#texform-normalize) | Normalize formulas with a transform profile |
| [`parse`](#texform-parse) | Parse formulas and inspect their syntax trees |
| [`tokenize`](#texform-tokenize) | Split the canonical serialization of formulas into typed tokens |
| [`info`](#texform-info) | Show knowledge-base records for a control sequence or an environment |
| [`packages`](#texform-packages) | List the built-in knowledge packages |
| [`argspec validate`](#texform-argspec-validate) | Check an xparse-style argument specification |
| [`serve`](#texform-serve-normalizer-protocol-v1) | Serve the normalizer protocol to another process |

`texform <COMMAND> --help` lists every option of a command.

### Global options

| Option | Meaning |
| --- | --- |
| `--packages <a,b,...>` | Knowledge packages to load. Defaults to the same six packages as the library (`base`, `ams`, `physics`, `textmacros`, `bboldx`, `boldsymbol`); `braket` is opt-in. An unknown name is a usage error with exit status `2`. `packages` and `argspec validate` do not use it. |

### Formula input

`normalize`, `parse`, and `tokenize` read formulas from one of three sources:

- The `LATEX` argument is one formula. Put `--` before a formula starting with `-`, for example `texform parse -- '-x^2'`. Unknown options are rejected rather than interpreted as formulas. Quote LaTeX with single quotes, such as `'\sqrt{a+b}'`, so the shell preserves backslashes.
- Without `LATEX`, all of stdin is one formula, minus one trailing newline (`\n` or `\r\n`). A formula with line breaks, such as an environment, stays whole.
- With `--lines`, every stdin line (ending in `\n` or `\r\n`) is one formula, and an empty line is an empty formula. Lines are processed as they arrive.

Input must be UTF-8.

### Output

`normalize` prints one line of LaTeX per formula, and `tokenize` prints one line containing a token list. `parse` prints a syntax tree; with `--lines`, trees are separated into blocks headed by `<line N>`. Use `--json --lines` for one JSON value per input line.

A failing formula does not stop the run:

- In text mode the error and diagnostics go to stderr. `parse` also shows any partial tree on stdout. Under `--lines`, failed normalization or tokenization writes an empty placeholder line, while tree blocks show `(failed)` when there is no tree to display.
- In JSON mode each formula produces one JSON value: successful normalization and parsing produce objects, successful tokenization produces an array, and any failure produces `{"ok":false,"error":{...}}`. A parse failure can also include a partial `syntax`. Processing errors, including recovered panics, do not write to stderr; usage and configuration errors still do.
- When the downstream reader closes stdout, formula processing stops immediately and exits successfully, without consuming the remaining stdin.

Diagnostics are colored only when stderr is a terminal and `NO_COLOR` is unset or empty.

### Exit status

| Status | Meaning |
| --- | --- |
| `0` | Every formula succeeded, or the lookup or validation succeeded |
| `1` | At least one formula failed (the rest of the input is still processed), `info` found nothing, or `argspec validate` rejected the specification |
| `2` | Usage, configuration, or I/O error: for example an unknown option or package, a missing `--profile`, an invalid `--config`, or unreadable or non-UTF-8 input. Output may be incomplete. |

### `--config`

`--config` takes a JSON object, inline or as `@FILE` to read it from a file. Its values are layered over the defaults, and nested objects may be partial. Unknown keys and a zero `rewrite.max_iterations` are rejected before reading formulas with exit status `2`, and the error names the offending path, such as `invalid --config: rewrite.enabld: unknown field ...`.

| Command | Config shape | Keys |
| --- | --- | --- |
| `normalize`, `tokenize --profile` | Normalize config over the profile defaults | `reject_unknown`, `abort_on_error`, `max_group_depth`, `lower_attributes`, `rewrite`, `finalize_ast`, `flatten_groups` |
| `parse`, `tokenize` | Parse config over the parser defaults | `reject_unknown`, `abort_on_error`, `max_group_depth` |

The normalize config has the same keys and resolves the same way as the `normalize` overlays of the Python binding (keyword arguments) and the JavaScript binding (camelCase keys), and as the `overrides` object of the serve protocol's `configure`. [`configure`](#configure) shows every key with its default value.

## Commands

### `texform normalize`

```bash
texform normalize --profile corpus 'a \over b'
texform normalize --profile authoring --config '{"rewrite":{"enabled":false}}' --lines < formulas.txt
texform normalize --profile corpus --report --json 'a \over b'
```

`--profile` is required: one of `authoring`, `faithful`, `corpus`, or `equiv`. The output is identical to `TransformEngine::normalize_with` in the Rust facade for the same packages, profile, and config. `--report` adds the transform report to each result and requires `--json`.

```console
$ printf '%s\n' 'a \over b' '\frac{a' '\dv{f}{x}' | texform normalize --profile corpus --lines 2>/dev/null
\frac { a } { b }

\frac { \mathrm { d } f } { \mathrm { d } x }
$ echo $?
1
```

### `texform parse`

Parses formulas without normalizing them. The default output is the compact `SyntaxNode` tree, the same display previously used by the parse example. `--verbose` prints the detailed tree as pretty-printed JSON; it conflicts with `--json`, which emits a compact result object for scripts. Success means a complete document: a partial document containing error nodes still exits with status `1`, but its tree remains visible.

| Parse result | Text mode (including `--verbose`) | `--json` |
| --- | --- | --- |
| Complete document | Syntax tree on stdout; diagnostics, if any, as warnings on stderr | `ok: true` with `syntax` and `diagnostics` |
| Partial document | Partial tree on stdout; error and diagnostics on stderr | `ok: false` with `error` and the partial `syntax` |
| No document | Error and diagnostics on stderr | `ok: false` with `error` |

```console
$ texform parse '\sqrt{a+b}'
Root(Math) [
  Command(\sqrt, known=true) [
    Arg(None)
    Arg(Mandatory, no_leading_space=false):
      Group(Math, Implicit) [
        Chars("a+b")
      ]
  ]
]
```

The diagnostic source is named `<argument>`, `<stdin>`, or `<line N>` after where the formula came from. Under `--lines`, each tree is headed by `<line N>` and followed by a blank line, including empty formulas and failures.

### `texform tokenize`

Lists tokens from the canonical serialization of each formula (`Document::to_tokenized_latex`), rather than lexing the original input. With `--profile`, the formula is normalized before tokenization. Partial documents fail as in `parse`.

The default output is a compact list: `[kind("text"), kind("text", mode=text), ...]`. Human-readable kinds abbreviate `character` to `char`, `control_sequence` to `control_seq`, and `delimiter` to `delim`; `text`, `raw`, and `error` retain their names. JSON keeps the full kind names. Math mode is implicit; only non-math tokens show their mode. Text uses JSON string quoting to preserve spaces and escape newlines, quotes, and backslashes. Empty formulas print `[]`. Each formula occupies one output line, without colors, column alignment, or manual line wrapping. Under `--lines`, failures write an empty placeholder line and diagnostics go to stderr.

```console
$ texform tokenize '\text{a b}'
[control_seq("\\text"), delim("{"), text("a b", mode=text), delim("}")]
```

`--json` directly emits an array of token objects, without an `ok` or `latex` wrapper. Each object preserves the bindings' `text`, `kind`, `mode`, `start_byte`, and `end_byte` fields. An empty formula produces `[]`. Under `--lines`, each successful input line produces one array on one output line; a failure produces the standard error object, keeping the correspondence with input lines.

Offsets refer to the canonical serialization, or the normalized serialization when `--profile` is supplied, not to the input string. Spaces between tokens belong to that serialization; joining token texts with spaces is not a way to reconstruct it, particularly in text mode.

### `texform info`

```bash
texform info '\frac'
texform info --env align
texform info --mode text '\textbf'
```

Looks up a name in the knowledge base of the loaded packages, in math mode unless `--mode text` is given.

- A control sequence such as `\frac` shows its command record and, when one exists, its character record. Character commands such as `\alpha` have both: the character record gives the Unicode value, and the command record shows that the name parses as a command without arguments. A few names, such as `\div` with `physics` loaded, are an ordinary command in one package and a character in another. When both records are shown, a note identifies the command record as the one used for parsing; the character record is separate metadata.
- `--env NAME` shows an environment record.
- A name that is neither a control sequence nor used with `--env` is a usage error. Characters are looked up by their control-sequence name; there is no lookup by Unicode character.

When nothing is found, the exit status is `1`.

```console
$ texform info '\alpha'
command:  \alpha
kind:     prefix
mode:     math
packages: base
argspec:  (no arguments)

character: \alpha
unicode:   α
mode:      math
variant:   italic
package:   base

Parsing uses the command record above. The character record is separate metadata.
```

### `texform packages`

```console
$ texform packages
ams         35 commands, 28 environments
base        267 commands, 7 environments
bboldx      12 commands, 0 environments
boldsymbol  1 command, 0 environments
braket      11 commands, 0 environments
physics     182 commands, 1 environment
textmacros  76 commands, 0 environments
```

### `texform argspec validate`

Checks an xparse-style argument specification with `validate_argspec` and describes each argument slot. An invalid specification is reported on stderr with exit status `1`.

```console
$ texform argspec validate 'm O{default} m'
valid: 3 arguments
  1. required, math content
  2. optional, math content
  3. required, math content
```

## JSON output

`--json` output uses the DTOs of `texform::bindings`, which the Python and JavaScript bindings also use, so field names and values match the Python binding exactly and the JavaScript binding up to camelCase. Member order is not significant.

| Command | Output line |
| --- | --- |
| `normalize` | `{"ok":true,"output":string,"report"?:object}` |
| `parse` | `{"ok":true,"syntax":object,"diagnostics":[object]}` |
| `tokenize` | `[{"text","start_byte","end_byte","kind","mode"}]` |
| Any formula command, on failure | `{"ok":false,"error":{"kind","message","diagnostics"},"syntax"?:object}` |
| `info` | `{"command"?:object,"character"?:object,"environment"?:object}`, or `null` when nothing is found |
| `packages` | `[{"name","commands","environments"}]` on one line |
| `argspec validate` | `{"valid","error","arg_count","parsed"}` |

- `error.kind` is `parse` (the formula did not parse into a complete tree), `transform` (a normalization step failed), or `internal` (an internal error, including a panic inside TeXForm, which fails only that formula). `error.diagnostics` lists parse diagnostics for `parse` failures and is empty otherwise; each diagnostic has `kind`, `message`, `span` (`start` and `end` byte offsets into the formula), `expected`, `found`, and `contexts`. Failures carry `syntax` only from `parse`, and only when a partial document exists.
- `syntax` is the serialized `SyntaxNode`, the tree format of `Document::to_syntax` in Rust, of `to_syntax()` in Python, and of `toSyntax()` in JavaScript.
- `report` is the transform report of `normalize_with_report`. Its fields are diagnostic and carry no compatibility promise.
- Each token is `{"text","start_byte","end_byte","kind","mode"}`. The offsets are UTF-8 byte offsets into the canonical serialization (after normalization when `--profile` is supplied); `kind` is `control_sequence`, `character`, `delimiter`, `text`, `raw`, or `error`, and `mode` is `math` or `text`.
- The `info` records are the results of `lookup_command`, `lookup_character`, and `lookup_env` in the bindings.

```console
$ texform normalize --profile corpus --json '\frac{a'
{"ok":false,"error":{"kind":"parse","message":"parse produced an incomplete document","diagnostics":[{"kind":"argument-validation","message":"unclosed brace argument","span":{"start":5,"end":6},"expected":[],"found":null,"contexts":[{"label":"command argument","span":{"start":5,"end":7}}]}]}}
$ texform tokenize --json 'x^2'
[{"text":"x","start_byte":0,"end_byte":1,"kind":"character","mode":"math"},{"text":"^","start_byte":2,"end_byte":3,"kind":"character","mode":"math"},{"text":"{","start_byte":4,"end_byte":5,"kind":"delimiter","mode":"math"},{"text":"2","start_byte":6,"end_byte":7,"kind":"character","mode":"math"},{"text":"}","start_byte":8,"end_byte":9,"kind":"delimiter","mode":"math"}]
```

## `texform serve`: normalizer protocol v1

`texform serve` is a long-running process that normalizes formulas on request. It speaks JSON-RPC 2.0 over stdin and stdout. The protocol is independent of TeXForm: any normalizer can implement it, and a client written against it can measure any implementation.

```bash
texform serve [--packages <a,b,...>]
```

`--packages` only sets the default for `configure` requests that omit `packages`.

### Transport

- Each message is one line of UTF-8 JSON terminated by `\n`. Messages must not contain raw newlines; JSON string escapes such as `\n` are fine.
- The server reads requests from stdin and writes only responses to stdout, one per line, flushing after each. stderr carries human-readable logs; clients must not parse it.
- Requests are processed one at a time in arrival order, and responses are written in the same order. Clients may pipeline requests or wait for each response.
- A request without an `id` member is a notification: it is executed, but no response is written, even on failure. `id` may be a string, a number, or `null`.
- Batch requests (JSON arrays) are not supported and receive a single `-32600` error.
- Blank lines are ignored.
- When stdin reaches end of file, the server finishes the requests it has read and exits with status `0`, whether or not `shutdown` was sent.

A session looks like this (`→` is client to server, `←` is server to client; lines are abbreviated):

```jsonc
→ {"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}
← {"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"serverInfo":{"name":"texform","version":"0.5.0","commit":"0123456789abcdef0123456789abcdef01234567","commitDate":"2026-09-22","dirty":false}}}
→ {"jsonrpc":"2.0","id":1,"method":"configure","params":{"id":"authoring-no-rewrite","config":{"profile":"authoring","overrides":{"rewrite":{"enabled":false}}}}}
← {"jsonrpc":"2.0","id":1,"result":{"resolved":{"profile":"authoring","packages":["ams","base","..."],"rewrite":{"enabled":false,"max_iterations":100},"...":"..."}}}
→ {"jsonrpc":"2.0","id":2,"method":"normalize","params":{"config":"authoring-no-rewrite","latex":"\\frac12","timing":true}}
← {"jsonrpc":"2.0","id":2,"result":{"output":"\\frac { 1 } { 2 }","timing":{"parse_ns":41250,"transform_ns":35980,"serialize_ns":2410}}}
→ {"jsonrpc":"2.0","id":3,"method":"normalize","params":{"config":"authoring-no-rewrite","latex":"\\frac{a"}}
← {"jsonrpc":"2.0","id":3,"error":{"code":1,"message":"parse produced an incomplete document","data":{"kind":"parse","diagnostics":[...]}}}
→ {"jsonrpc":"2.0","id":4,"method":"shutdown","params":{}}
← {"jsonrpc":"2.0","id":4,"result":null}
```

### Methods

| Method | `params` | `result` |
| --- | --- | --- |
| `initialize` | `{}` | `{"protocolVersion":1,"serverInfo":{...}}` |
| `configure` | `{"id":string,"config":{"profile":string,"packages"?:[string],"overrides"?:object}}` | `{"resolved":object}` |
| `normalize` | `{"config":string,"latex":string,"timing"?:bool}` | `{"output":string,"timing"?:object}` |
| `shutdown` | `{}` | `null` |

`params` must be an object; `initialize` and `shutdown` also accept omitted `params`. Unknown members of `params` itself are ignored, while unknown members of `config` and `overrides` are rejected (see [Versioning](#versioning)).

#### `initialize`

Must be the first request. Any other method received earlier fails with `-32002`. Repeating `initialize` is harmless and returns the same result.

`serverInfo` identifies the build (see [Build identity](#build-identity)):

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string | `"texform"` |
| `version` | string | Crate version |
| `commit` | string or `null` | Full commit hash, or `null` without git information |
| `commitDate` | string or `null` | Committer date of `commit` (`YYYY-MM-DD`) |
| `dirty` | bool | Build inputs differed from `commit`; `false` without git information |

#### `configure`

Builds a normalizer from `config` and stores it under `id` for later `normalize` requests. Configuring an existing `id` replaces its entry; a failed `configure` leaves the previous entry unchanged.

- `profile`: one of `authoring`, `faithful`, `corpus`, `equiv`. It selects the rule set and the default config.
- `packages`: knowledge packages to load. When omitted, the server's `--packages` value applies, and without that, the same six runtime packages as the library (`braket` is opt-in). Clients that need a fixed selection should specify it explicitly and record `resolved`.
- `overrides`: config values layered over the profile defaults. It accepts any subset of the keys in `resolved` other than `profile` and `packages`, with the same nesting; nested objects may also be partial.

`resolved` is the complete effective config, suitable for recording alongside results:

```json
{
  "profile": "authoring",
  "packages": ["ams", "base", "bboldx", "boldsymbol", "physics", "textmacros"],
  "reject_unknown": false,
  "abort_on_error": false,
  "max_group_depth": 128,
  "lower_attributes": { "enabled": true },
  "rewrite": { "enabled": false, "max_iterations": 100 },
  "finalize_ast": { "enabled": true },
  "flatten_groups": { "enabled": true, "preserve_rendered_spacing": true }
}
```

`packages` lists the loaded set without duplicates in catalog order, regardless of the order requested; the load order is fixed by TeXForm. Key order within `resolved` is not significant.

An unknown profile, an unknown package, or an invalid override (including `rewrite.max_iterations = 0`, even when rewriting is disabled) fails with `-32602` before registering the configuration.

#### `normalize`

Normalizes `latex` with the configuration stored under `config`. The result is identical to `TransformEngine::normalize_with` in the Rust facade for the same input and effective config. A formula that fails to normalize is a normal outcome, reported as error code `1` (see [Errors](#errors)). An unknown `config` id fails with `-32602`.

With `timing: true`, the response also carries per-stage durations (see [Timing](#timing)).

#### `shutdown`

Returns `null`. Every later request fails with `-32600`; the server then only waits for end of file on stdin.

### Errors

Error responses carry a JSON-RPC error object `{"code","message","data"?}`. There are two layers: protocol errors mean the request itself was invalid, while code `1` is a per-formula result that callers should treat as data.

| `code` | Condition | `data` |
| --- | --- | --- |
| `-32700` | The line is not valid UTF-8 JSON; the response `id` is `null` | none |
| `-32600` | Not a valid JSON-RPC 2.0 request, a batch array, or any request after `shutdown` | none |
| `-32601` | Unknown method | none |
| `-32602` | Malformed params, unknown profile or package, invalid `overrides`, or unconfigured `config` id | `{"message"}` |
| `-32002` | Request before `initialize` | none |
| `1` | The formula failed to normalize | `{"kind","diagnostics","timing"?}` |

- For `-32602`, `message` is `"Invalid params"` and `data.message` explains the problem, including the path of an offending field such as `config.overrides.rewrite.enabld`.
- For code `1`, `message` describes the failure and `data.kind` classifies it:
  - `parse`: the formula did not parse into a complete tree.
  - `transform`: a normalization step failed.
  - `internal`: an internal error, including a panic inside the server. The server stays usable after a panic.
- `data.diagnostics` lists parse diagnostics for `parse` failures and is empty otherwise. Each diagnostic has `kind` (a stable kebab-case identifier or `null`), `message`, `span` (`start` and `end` byte offsets into `latex`), `expected`, `found`, and `contexts`.
- The server applies no timeouts. A client that needs one should kill and restart the process.

### Timing

With `timing: true`, a successful `normalize` result contains `timing` with three members, each an integer number of nanoseconds:

| Member | Stage |
| --- | --- |
| `parse_ns` | Parsing `latex` into a complete tree |
| `transform_ns` | Normalizing the tree |
| `serialize_ns` | Serializing the normalized tree |

- Timings cover only the server's internal work, not JSON decoding, encoding, or I/O.
- A code `1` failure carries `timing` in `error.data.timing` with only the stages that ran: a parse failure has only `parse_ns`, and a transform failure has `parse_ns` and `transform_ns`. Failures caused by a panic carry no timing.
- Requesting timing never changes `output` or the error classification.
- Whether a server supports timing is decided from successful responses; an implementation without timing support ignores the member.

### Versioning

- `protocolVersion` increases only when method semantics or required fields change incompatibly. New optional fields and new methods keep the version.
- Clients must refuse to continue with a `protocolVersion` they do not recognize.
- Servers ignore unknown members of `params`, so clients can send newer optional fields to older servers. Unknown members of `config` and `overrides` are rejected instead, so a configuration an older server cannot honor fails at `configure` rather than silently producing different results.
