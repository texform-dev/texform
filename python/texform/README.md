# texform

Python bindings for [TeXForm](../../README.md), a LaTeX formula parser, editor, and normalizer built on a structured command knowledge base.

```bash
pip install texform
```

## Quick start

```python
import texform

# Normalize a formula into a canonical form chosen by profile.
engine = texform.TransformEngine(profile="corpus")
result = engine.normalize(r"a \over b")
assert result["normalized"] == r"\frac { a } { b }"

# Parse through the engine, transform the live document in place, then serialize.
parsed = engine.parse(r"a \over b")
if parsed["document"] is not None:
    document = parsed["document"]
    engine.transform(document)
    assert document.to_latex() == r"\frac { a } { b }"
```

Profiles select the normalization target: `"authoring"`, `"faithful"`, `"corpus"`, and `"equiv"`.

## Python-specific notes

- `Parser.parse` returns a dict with a `document` value (or `None`) plus a `diagnostics` list — the same three-state contract as the Rust API.
- All names follow Python conventions: methods and dict keys are snake_case (`to_latex`, `validate_argspec` returns `arg_count`).
- Parse and edit errors raise structured exceptions (`texform.ParseError` and friends); no Rust panic ever crosses the boundary.
- The package ships `py.typed` and `.pyi` stubs, so type checkers and IDE completion work out of the box.
- Wheels are abi3 and require Python 3.10 or newer.

## Learn more

The Python API mirrors the Rust facade one-to-one. For the full picture — the editable document tree, transform profiles, and the architecture — see the [repository README](../../README.md).

<!-- Full documentation: https://texform.dev (docsite goes live after 0.1.0) -->

## License

Apache-2.0.
