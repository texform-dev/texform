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

## Constructing formulas

Constructors return detached nodes. Attach the finished subtree with `append_child`. Arguments accept `Node`, source `str`, star `bool`, `None`, or immutable `Paired(value, open, close)`. Strings contain source inside the argument boundaries. Supply either every slot or only required slots; `None` omits optional slots and disables a star.

```python
doc = texform.Document()
frac = doc.create_command("frac", ["a+b", "c"])
scripted = doc.create_scripted(frac, sub="i", sup="2")
paren = doc.create_delimited_group("(", ")", [scripted])
doc.append_child(doc.root(), paren)
print(doc.to_latex())
```

Groups accept children as nodes or source strings; a string is parsed and all its nodes are spliced into the list, so one string may yield several children. Environment bodies accept a group, a list of nodes or source strings, source, or `None` for an empty body. `parse_fragment(source, mode="math")` produces a detached group. Constructors accept `mode=` to specify the detached subtree's context; `create_group(mode, children)` uses that mode for both group content and context. Delimiters use strings: `"("`, `"\\langle"`, or `"."` for no delimiter.

Invalid construction, edits, and strict `Document.from_syntax` imports raise `ConformanceError`, an `EditError` subclass with `path` and stable `rule` attributes. Invalid source arguments raise `ParseError` with `diagnostics`. A rejected operation leaves the document unchanged. `to_latex` and `serialize` check only structure, not conformance to a knowledge base. Rename environments with `set_env_name`; `set_command_name` applies to command nodes.

## Editing and locating nodes

`node.path()` describes its current location, such as `root.child.0.base`; detached nodes return `None`. Save a path before copying a document, then use `copy.node_at(path)` to locate the corresponding node. Missing paths raise `EditError`. `node.slot()` returns a dictionary with a snake_case `kind` and optional `index`, while `node.is_known()` reports knowledge status for command-like nodes and environments.

```python
doc = texform.Parser().parse("x_i")["document"]
base = doc.node_at("root.child.0.base")
scripted = doc.set_superscript(base, "2")
assert scripted == doc.node_at("root.child.0")
doc.set_subscript(base, None)
doc.set_superscript(base, None)  # Both scripts absent: collapse to the base.
```

`set_subscript` and `set_superscript` accept the same argument inputs as constructors and return the resulting wrapper or base. `set_arg` fills or clears optional slots, toggles stars, and preserves existing paired boundaries when given an ordinary value. `set_arg_delimiters` changes paired boundaries; `set_delimiters` changes a delimited group; `set_prime_count` requires a positive count.

`clone_node(node)` makes a detached deep copy within the document. `import_node(node)` takes a handle from any document, copying it and checking the subtree against the destination knowledge base. Error subtrees cannot be imported from another document. The source remains unchanged, and a same-document import behaves as a clone.

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

## Columnar export

`document.to_columnar()` flattens the tree into columns for bulk structural analysis with Arrow or DataFrame tools. It carries information comparable to `document.to_syntax()`: `to_syntax()` returns a nested snapshot, while `to_columnar()` returns columnar data.

`document.to_columnar()` exports the attached tree once as `{"nodes": ..., "args": ...}`, with one Python list per column. Nodes use preorder; arguments include every slot, including absent optional slots and scalar values. Each table can be passed directly to `pyarrow.table()` without a TeXForm dependency on PyArrow:

```python
import pyarrow as pa

tables = texform.Parser().parse(r"\sqrt{x}")["document"].to_columnar()
nodes = pa.table(tables["nodes"])
arguments = pa.table(tables["args"])
```

`parent`, `slot_index`, and `content` use `-1` when no index applies; other missing fields are `None`. Node `mode` describes its context, while `value_kind` distinguishes math, text, and operator-name arguments. `open` and `close` retain actual paired-argument boundaries. Row indices belong to the snapshot, detached subtrees are excluded, and documents containing errors remain exportable.

## License

Apache-2.0.
