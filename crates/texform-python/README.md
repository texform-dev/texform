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

For uv projects that depend on this checkout as a local path dependency, `uv run` and `uv sync` automatically rebuild the extension when its Rust sources, Cargo manifests, or knowledge resources change. The root `tool.uv.cache-keys` configuration tracks these build inputs; extend it when adding native dependencies or build resources. Automatic rebuilding requires uv environment synchronization, so `uv run --no-sync` and direct Python invocations bypass it.

## API changes and validation

Keep the Rust binding, `python/texform/__init__.pyi`, package exports, and user-facing examples consistent. Shared configuration input and DTO definitions live in `crates/texform/src/bindings/`; changes there may require JavaScript validation too.

Plain `normalize` returns `str` and plain `transform` returns `None`. `normalize_with_report` and `transform_with_report` reuse those config parsers and return the diagnostic DTO. The plain path does not build that DTO. `_normalize_with_flatten_groups_guards` keeps its name and always returns a report.

Run the embedded Python tests, then check the rebuilt extension from the repository root:

```bash
uv run cargo test -p texform-python
uv run python -c 'import texform; texform.Parser(); print("texform ok")'
```

The Rust test suite exercises Python objects, config conversion, and exception behavior through PyO3. The import check verifies the installed extension loads; rebuild with `maturin develop` after changing native code. CI also builds and imports a wheel, but that smoke check does not replace the behavior tests.
