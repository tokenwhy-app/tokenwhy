# tokenwhy-co2-data

Third-party code and data behind tokenwhy's CO2e and water estimates, kept apart from tokenwhy's own code.

- `src/ecologits/`: Rust port of [EcoLogits](https://github.com/mlco2/ecologits) 0.11.3 (MPL-2.0, GenAI Impact).
  `models.json` is unmodified upstream data; the licence text is in `src/ecologits/LICENSE`.
- `src/jegham.json`: emission factors of [claude-carbon](https://github.com/gwittebolle/claude-carbon) (MIT,
  Gaetan Wittebolle), after Jegham et al. 2025.

Full credits and licence texts: `NOTICE` at the repository root.
