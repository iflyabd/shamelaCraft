# Contributing

- Clean-room only: do not copy code, text, icons, or data from proprietary
  readers. Interoperability via documented on-disk formats is fine.
- `cargo fmt --all` must be clean; `cargo clippy --workspace --all-targets`
  must pass with `-D warnings`; add unit tests for new `shamela-db` /
  `shamela-index` behavior (in-memory fixtures only — never commit
  proprietary database files).
- Keep the GUI dependency-light and RTL-first; every new string should have
  an Arabic label.
- Open an issue before large features so parity work stays coordinated.
