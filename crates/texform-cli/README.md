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

## Global options

| Option | Meaning |
| --- | --- |
| `--packages <a,b,...>` | Knowledge packages to load. Defaults to all built-in packages (`base`, `ams`, `braket`, `physics`, `textmacros`, `bboldx`, `boldsymbol`). An unknown name is a usage error with exit status `2`. |

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
- `packages`: knowledge packages to load. When omitted, the server's `--packages` value applies, and without that, all built-in packages.
- `overrides`: config values layered over the profile defaults. It accepts any subset of the keys in `resolved` other than `profile` and `packages`, with the same nesting; nested objects may also be partial.

`resolved` is the complete effective config, suitable for recording alongside results:

```json
{
  "profile": "authoring",
  "packages": ["ams", "base", "bboldx", "boldsymbol", "braket", "physics", "textmacros"],
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

An unknown profile, an unknown package, or an invalid override fails with `-32602`.

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
