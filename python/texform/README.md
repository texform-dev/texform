# texform

Python bindings for [TeXForm](https://github.com/texform-dev/texform), a LaTeX formula parser, editor, and normalizer built on a structured command knowledge base.

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

# Parse, inspect, edit, and serialize back to LaTeX.
parsed = texform.Parser().parse(r"\frac{x}{y}")
if parsed["document"] is not None:
    print(parsed["document"].to_latex())
```

Profiles select the normalization target: `"authoring"`, `"faithful"`, `"corpus"`, and `"equiv"`.

## Python-specific notes

- `Parser.parse` returns a dict with a `document` value (or `None`) plus a `diagnostics` list — the same three-state contract as the Rust API.
- All names follow Python conventions: methods and dict keys are snake_case (`to_latex`, `validate_argspec` returns `arg_count`).
- Parse and edit errors raise structured exceptions (`texform.ParseError` and friends); no Rust panic ever crosses the boundary.
- The package ships `py.typed` and `.pyi` stubs, so type checkers and IDE completion work out of the box.
- Wheels are abi3 and require Python 3.10 or newer.

## Learn more

The Python API mirrors the Rust facade one-to-one. For the full picture — the editable document tree, transform profiles, and the architecture — see the [GitHub repository](https://github.com/texform-dev/texform).

<!-- Full documentation: https://texform.dev (docsite goes live after 0.1.0) -->

## License

Apache-2.0.
