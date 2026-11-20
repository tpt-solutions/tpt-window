# Security policy

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through GitHub's
"Report a vulnerability" button on the repository's Security tab, or by contacting the maintainers
at TPT Solutions. Include what you ran, what you expected, and what happened. You will get an
acknowledgement, and we will agree a disclosure date with you before anything is published.

## What is in scope

- **Input signing (`tpt-window-archon`).** Input events are signed with Ed25519 by the router, and
  the application verifies them with the public key only. A way to forge, replay, reorder or move
  an event between windows without the signing key is a vulnerability. So is a way to make a
  verifier accept a signature made for another purpose (the domains are separate).
- **Session logs.** A signed log must fail verification when a record is changed, dropped,
  reordered, inserted, or when the log is cut short. Any way around that is a vulnerability.
- **Capabilities.** A holder of a read-only capability must not be able to send input, and a
  revoked capability must stop working at once. A window must not receive events it has no
  capability for.
- **Shared memory (`tpt-window-shm`, `tpt-window-memfd`).** Reading or writing outside the mapping,
  or a frame that a peer can make the consumer misinterpret in a way that reads memory it should
  not.
- **Parsers.** The wire protocol, the event codec and the log readers must not panic or read out
  of bounds on hostile input. The fuzz targets in `fuzz/` cover them.

## What is not in scope

- Bugs that need a process already running as the same user, or local administrator access.
- The windowing behaviour of a platform (for example, a window manager that ignores a hint).
- The Win32 shared-memory name squatting that remains until an explicit DACL is added. It is on the
  open-risk list in the repository, and it is known.

## Supported versions

Nothing is published yet. Security fixes land on `master`.
