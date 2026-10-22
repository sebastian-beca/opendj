# Design

## Context
Phase 1 (opendj #36) loads executables from `<config>/addons/` and speaks JSON lines over stdio. The full design is in the project note `notes/addon-repositories.md`.

## Decisions
- **Index format:** one `opendj-addons.json` per repository, hosted anywhere; on GitHub a release asset at `/releases/latest/download/opendj-addons.json`. Chosen over a registry service so anyone can publish.
- **Trust:** checksums always; an ed25519 key pinned on first add (trust on first use). Add-ons are not sandboxed; openDJ shows publisher, declared hosts and "unsupported".
- **Secrets:** tokens in the macOS Keychain / Linux Secret Service, never in settings.json.
- **Versions:** `initialize` carries the protocol version; an index entry names the openDJ versions it needs, and too-new add-ons show but can't install.

## Risks / Trade-offs
- Malicious add-on from an unsupported repository → warning before install, publisher and hosts shown, remove in one click.
- GitHub rate limits on index fetches → cache the index, check at most daily.
