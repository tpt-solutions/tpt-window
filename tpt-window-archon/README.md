# tpt-window-archon

The capability bridge between [tpt-window](../tpt-window) and
[tpt-archon](https://github.com/tpt-solutions/tpt-archon). It works without archon.

## What it provides

- **`capability`**: maps tpt-window's scopes (`CapabilityToken`) onto archon's
  `Resource::Channel` / `Resource::Device` capabilities. Each window's events are a channel.
  Global input and any-display placement each need one device read. Child-window scopes are
  rejected, not granted.
- **`envelope`**: input events signed with Ed25519 by the trusted router. Applications verify
  with the public key only. Each signature covers the window id, a per-window sequence number
  and a canonical encoding of the event, so altered events, moved events and replays fail.
- **`router`**: `WindowRouter`, which checks a capability on every delivery and every receive,
  signs what it delivers, and keeps one inbox per window.

The router is generic over an `Authority` trait. In tpt-archon that trait is implemented by a
small adapter over `CapabilityIssuer::authorizes`. That adapter belongs in tpt-archon, and
this crate does not depend on it. The tests use a mock authority that follows archon's
semantics: a token is live only while not revoked, and `ReadWrite` covers both rights.

## Not yet done

- The tpt-archon adapter (`SharedIssuer` → `Authority`).
- Key storage: the signing key is passed in by the caller. The key should come from archon's
  protected store.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Plan and open decisions: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
