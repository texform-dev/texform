# texform-regression

Corpus regression harness for TeXForm. Internal tooling, not published to crates.io and not part of the public API.

It runs the parser and the transform pipeline against large real-world formula corpora and provides three binaries: `parser_regression` (parser error-rate regression against tracked baselines), `transform_contract` (full-pipeline eliminated-form contract checks), and `counter_dump` (per-formula target counter data products).

Datasets, run commands, and result layouts are documented in [`regression/README.md`](../../regression/README.md).
