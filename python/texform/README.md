# texform

> **The LaTeX math toolkit**

Python bindings for [TeXForm](https://texform.dev), which parses, edits, and normalizes LaTeX math using a knowledge base of command and environment specifications.

[Website](https://texform.dev) · [Documentation](https://texform.dev/docs) · [Python API reference](https://texform.dev/api/python) · [Playground](https://play.texform.dev) · [Changelog](https://texform.dev/docs/changelog)

```bash
pip install texform
```

## Quick start

```python
import texform

# Normalize a formula into a canonical form chosen by profile.
engine = texform.TransformEngine(profile="corpus")
normalized = engine.normalize(r"a \over b")
assert normalized == r"\frac { a } { b }"

# Parse through the engine, transform the live document in place, then serialize.
parsed = engine.parse(r"a \over b")
if parsed["document"] is not None:
    document = parsed["document"]
    engine.transform(document)
    assert document.to_latex() == r"\frac { a } { b }"
```

Profiles select the normalization target: `"authoring"`, `"faithful"`, `"corpus"`, and `"equiv"`. See [Transforms](https://texform.dev/docs/transforms) for what each profile changes.

## Command line

The package also installs the `texform` command-line tool, which is also available as `python -m texform`. To run it without installing, use `uvx texform`:

```console
$ uvx texform normalize --profile corpus 'a \over b'
\frac { a } { b }
```

See the [command-line guide](https://texform.dev/docs/command-line) for all commands.

## Python-specific notes

- The Python API mirrors the Rust facade one-to-one, with Python naming: methods and dict keys are snake_case (`to_latex`, `validate_argspec` returns `arg_count`).
- `Parser.parse` returns a dict with a `document` value (or `None`) plus a `diagnostics` list — the same three-state contract as the Rust API.
- `normalize` returns `str`; `transform` updates the document and returns `None`.
- `config` is a complete configuration class (`ParseConfig` / `TransformConfig`). Other keyword arguments are overlays (`normalize(src, rewrite={"enabled": False})`), and serialize options are keywords too (`document.to_latex(style="compact")`).
- Pass the same `KnowledgeBase` to each parser, engine, or document that should interoperate; omitting it uses a process-wide default.
- Parse, edit, and configuration errors raise structured exceptions (`texform.ParseError`, `texform.ConfigError`, and friends); no Rust panic ever crosses the boundary.
- The package ships `py.typed` and `.pyi` stubs, so type checkers and IDE completion work out of the box.
- Wheels are abi3 and require Python 3.10 or newer.

## Learn more

- [Quick start](https://texform.dev/docs/quick-start) — profiles, the document tree, and parse results in ten minutes
- [Edit a document](https://texform.dev/docs/edit-a-document) — construct, edit, and locate nodes in the document tree
- [Analyze formula structure at scale](https://texform.dev/docs/analyze-structure) — export documents as columnar tables for Arrow or DataFrames
- [Normalize a corpus](https://texform.dev/docs/normalize-a-corpus) — run one engine over a dataset and aggregate statistics
- [Python API reference](https://texform.dev/api/python) — every class, method, and exception

## License

Apache-2.0.
