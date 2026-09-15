# texform-python

PyO3 bindings that back the [`texform` package on PyPI](https://pypi.org/project/texform/). Not published to crates.io.

This crate compiles to the `texform._native` extension module (abi3, Python ≥ 3.10). The Python package source — `__init__.py`, type stubs, and the PyPI README — lives in [`python/texform/`](../../python/texform/). Bindings layer strictly on top of the `texform` facade: live `Document` and `Node` handles delegate to the shared Rust core, and errors surface as structured Python exceptions.

## Local development

Build the extension into a local virtualenv from the repository root:

```bash
uv sync --dev
uv run maturin develop
```

Release wheels are built with `maturin build --release`; packaging metadata lives in the root `pyproject.toml`.

## API changes and validation

Keep the Rust binding, `python/texform/__init__.pyi`, package exports, and user-facing examples consistent. Shared configuration input and DTO definitions live in `crates/texform/src/bindings/`; changes there may require JavaScript validation too.

Run the embedded Python tests, then check the rebuilt extension from the repository root:

```bash
uv run cargo test -p texform-python
uv run python -c 'import texform; texform.Parser(); print("texform ok")'
```

The Rust test suite exercises Python objects, config conversion, and exception behavior through PyO3. The import check verifies the installed extension loads; rebuild with `maturin develop` after changing native code. CI also builds and imports a wheel, but that smoke check does not replace the behavior tests.
