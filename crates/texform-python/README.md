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
