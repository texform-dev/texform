# texform

Python bindings for TeXForm's LaTeX parser, serializer, and transform engine.

Build the native extension from the repository root:

```bash
uv sync --dev
uv run maturin develop
```

```python
import texform

parser = texform.Parser()
result = parser.parse(r"\frac{x}{y}")

if result["document"] is not None:
    print(result["document"].to_latex())

document = texform.Document()
root = document.root()
x = document.create_char("x")
document.append_child(root, x)

print(document.to_latex())

engine = texform.Engine(profile="authoring")
normalized = engine.normalize(r"a \over b")

print(normalized["normalized"])
print(texform.validate_argspec("m o"))
```

`serialize(node, options)` remains as a compatibility helper for `SyntaxNode` snapshots. New code should use `texform.Document.from_syntax(node).to_latex(options)` or `document.to_latex(options)`.

See the repository [`README.md`](../../README.md) and [`ARCHITECTURE.md`](../../ARCHITECTURE.md) for the full API and design.
