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
normalized = engine.normalize(r"a \over b")
assert normalized == r"\frac { a } { b }"

# Parse through the engine, transform the live document in place, then serialize.
parsed = engine.parse(r"a \over b")
if parsed["document"] is not None:
    document = parsed["document"]
    engine.transform(document)
    assert document.to_latex() == r"\frac { a } { b }"
```

Profiles select the normalization target: `"authoring"`, `"faithful"`, `"corpus"`, and `"equiv"`.

## Shared knowledge and document copies

Knowledge configuration belongs to `KnowledgeBase`; pass the same instance to each parser, engine, or document that should interoperate. Omitting it uses a process-wide default. Separately constructed knowledge bases have different identities even with identical package lists.

```python
kb = texform.KnowledgeBase(["base", "ams"])
parser = texform.Parser(kb)
engine = texform.TransformEngine("equiv", kb)
document = parser.parse("x")["document"]
variant = document.copy()  # copy.copy and copy.deepcopy also copy the tree.
assert variant.knowledge_base() == kb
assert variant.root() != document.root()
assert document.root() == document.root()
assert document.root().document() is document
engine.transform(variant)
records = kb.commands("math")  # Sorted copied records; knowledge is immutable.
```

`items`, `remove_commands`, `remove_environments`, and `remove_delimiter_controls` are keyword options on `KnowledgeBase`. Knowledge queries have moved from `Parser` and `TransformEngine` to `KnowledgeBase`. `commands(mode)`, `environments(mode)`, `characters(mode)`, `delimiters()`, and `packages()` enumerate its contents. `KnowledgeBase([])` constructs empty knowledge. `Document(kb, mode="text")` creates a text root, and `Document.from_syntax(snapshot, kb)` rebuilds a snapshot with shared knowledge. `count_targets(source, knowledge_base=kb)` uses the same knowledge for analysis.

## Python-specific notes

- `normalize` returns `str`. `transform` updates the document and returns `None`.
- `normalize_with_report` returns `{"normalized", "report"}`. `transform_with_report` returns the report dict. Both use the same config object and keyword overlays as the plain methods. Output text and errors follow the ordinary transform contract. Report fields, phase divisions, and counters are diagnostic and are not a stable compatibility promise.
- `Parser.parse` returns a dict with a `document` value (or `None`) plus a `diagnostics` list — the same three-state contract as the Rust API.
- All names follow Python conventions: methods and dict keys are snake_case (`to_latex`, `validate_argspec` returns `arg_count`).
- `config` is a complete configuration class (`ParseConfig` / `TransformConfig`). Other keyword arguments are overlays (`normalize(src, rewrite={"enabled": False})`). Saved dicts expand with `**saved_overrides`. Nested override values must be dicts, not config class instances.
- `parser.default_parse_config()`, `engine.default_parse_config()`, and `engine.default_transform_config()` return the complete defaults actually in force. Nested assignment sticks: `cfg = engine.default_transform_config(); cfg.rewrite.enabled = False`.
- Serialize options are keywords too: `document.to_latex(script_order="sup_first")`.
- Unknown keys, wrong types, and arrays where an object is expected raise `ConfigError` with a field path. `None` / omitted means not set.
- Parse and edit errors raise structured exceptions (`texform.ParseError` and friends); no Rust panic ever crosses the boundary.
- The package ships `py.typed` and `.pyi` stubs, so type checkers and IDE completion work out of the box.
- Wheels are abi3 and require Python 3.10 or newer.

## Learn more

The Python API mirrors the Rust facade one-to-one. For the full picture — the editable document tree, transform profiles, and the architecture — see the [repository README](../../README.md).

## License

Apache-2.0.
