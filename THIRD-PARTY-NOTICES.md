# Third-Party Notices

tyda is licensed under the MIT License (see [LICENSE](LICENSE)). It builds on
the following third-party components, each under its own license. Their
copyright and license notices are retained as required.

## Bundled / linked components

- **ruby/rbs** — the RBS C parser (compiled into the binary via the
  `ruby-rbs-sys` crate) and the standard-library RBS type definitions
  (`core` / `stdlib`, fetched from the `rbs` gem and bundled into the wasm
  playground).
  License: BSD-2-Clause or the Ruby License (dual).
  Copyright (C) 2019 Soutaro Matsumoto. <https://github.com/ruby/rbs>

- **prism** — the Ruby source parser (via the `ruby-prism` / `ruby-prism-sys`
  crates), compiled into the binary.
  License: MIT.
  Copyright 2022-present, Shopify Inc. <https://github.com/ruby/prism>

## Cargo dependencies

The remaining Rust dependencies are under permissive licenses (predominantly
`MIT OR Apache-2.0`, plus MIT, Apache-2.0, BSD, ISC, Unicode-3.0, Zlib). A full
per-crate manifest can be generated with `cargo about` or `cargo deny` for a
release build.
