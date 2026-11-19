# Contributing

- Unless you state otherwise, any contribution intentionally submitted for
  inclusion is dual-licensed MIT OR Apache-2.0, without additional terms.
- Dependencies must be MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib or Unicode-3.0.
  No GPL/LGPL/AGPL/MPL/SSPL/BSL. Run `cargo deny check` before opening a PR.
- The event hot path (`poll_events`, `EventSink::push`) must not allocate.
- Run `cargo fmt`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace`.
- Add `// SPDX-License-Identifier: MIT OR Apache-2.0` to new source files.
