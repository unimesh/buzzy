# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Buzzy

## Project overview

Buzzy is a daemon-based real-time collaboration layer for files. The daemon (`buzd`) watches files on disk, maintains CRDT state, syncs with peers over QUIC, and exposes a JSON-RPC socket for editor plugins.

## Current state — read this first

**The repository is documentation-only. No Rust code exists yet.** There is no `Cargo.toml`, no `crates/`, no workspace. Everything below the "Architecture" heading describes a design to be built, not code to be read. `cargo build` and `bzz` will not work until the workspace is scaffolded.

The specification is the source of truth. When implementing, treat these as authoritative:

| Doc | Use it for |
|-----|-----------|
| `docs/mvp-lld.md` | **The implementation spec.** Crate layout, trait definitions (`CrdtEngine`/`CrdtDocument`/`SyncSession`), full JSON-RPC method list, QUIC wire format (message type IDs), ingestion-gate state machine, config schemas, performance targets, 12-week task breakdown. Start here for any coding task. |
| `docs/design.md` | Architecture rationale, permissions model, external-edit handling |
| `docs/crdt-engine-decision.md` | Why Automerge-rs, and the conditions under which to revisit it |
| `docs/sync-topology.md`, `docs/protocol-research.md` | Transport/CRDT/sync alternatives and why each was chosen |
| `docs/research/` | Raw research notes (algorithms, protocols, formats, identity) |

Diagrams live in `docs/assets/buzzy/` as paired `.puml` (source) + `.png`. Edit the `.puml` and regenerate the `.png`; never hand-edit the PNG.

**Resolved:** the daemon keeps everything under `~/.buzd/` — `config.toml`, `identity.key`, `peers.toml`, `buzd.sock`, `buzd.pid`. (Earlier drafts of `docs/mvp-lld.md` used `~/.buzzy/` for daemon config and `/tmp/buzd.sock` for the socket; these are now unified.) Do not reintroduce `~/.buzzy/` for daemon config or `/tmp/` for the socket. Note the distinct, still-correct `.buzzy/` **per-vault** directory that holds CRDT state inside each vault — that one keeps its name.

## Naming

| Name | What it refers to |
|------|------------------|
| **Buzzy** | The project, the repo, the community |
| **buzd** | The daemon process (background, long-running) |
| **bzz** | The CLI tool (`bzz init`, `bzz start`, `bzz share`) |
| **Buzz** | The commercial product (future — not in this repo) |
| **.buzzy/** | Per-vault CRDT state directory |
| **~/.buzd/** | Daemon-level config directory |

## Architecture

```
Editor Plugins (JSON-RPC over Unix socket)
       ↕
buzd (daemon)
  ├── CRDT Engine (pluggable via CrdtDocument trait)
  ├── File Watcher + Ingestion Gate
  ├── Network (mDNS + QUIC)
  └── Sync Protocol (via SyncSession trait)
       ↕
.buzzy/ (per-vault CRDT state)
```

## Key design principles

1. **Files are canonical.** The `.md` / `.rs` / `.mid` file on disk is truth. The CRDT sidecar enables collaboration but is not authoritative. If they diverge, the file wins.
2. **CRDT engine is pluggable.** The daemon depends on traits (`CrdtEngine`, `CrdtDocument`, `SyncSession`), never on Automerge types directly. Only `buzzy-crdt-automerge` imports Automerge.
3. **Editor plugins are thin.** ~200 lines of JSON-RPC over Unix socket. They send position-based ops and receive position-based updates. No CRDT logic in plugins.
4. **Git coexistence.** The ingestion gate detects `.git/index.lock`, `MERGE_HEAD`, rebase dirs. During git operations, buzd pauses ingestion and does not broadcast intermediate states.
5. **No server required.** LAN sync via mDNS + direct QUIC. No cloud dependency for the open-source version.

## Crate structure (planned)

```
crates/
├── buzzy-crdt/                  (traits — CrdtEngine, CrdtDocument, SyncSession)
├── buzzy-crdt-automerge/        (Automerge implementation of traits)
├── buzzy-daemon/                (buzd — runtime, socket server, watcher, registry)
├── buzzy-net/                   (networking — mDNS, QUIC, sync)
├── buzzy-cli/                   (bzz — CLI binary)
└── buzzy-protocol/              (shared types — RPC messages, operations, wire format)
```

## File layout

```
~/.buzd/                         daemon config
  identity.key                   Ed25519 keypair
  config.toml                    configuration
  peers.toml                     known peers
  buzd.sock                      Unix socket (runtime)

vault/.buzzy/                    per-vault CRDT state
  .gitignore                     contains "*" (self-gitignored)
  index.json                     path ↔ UUID mapping
  state/<uuid>.bin               Automerge binary per document
```

## Build, test, run

These are the intended commands once the workspace exists (see `docs/mvp-lld.md` for the crate layout to scaffold). None work until the code is written.

```bash
cargo build --workspace          # build all crates
cargo test --workspace           # run all tests
cargo test -p buzzy-crdt <name>  # run a single test by name in one crate
cargo fmt                        # format
cargo clippy --workspace         # lint

bzz init ~/my-vault              # create .buzzy/ in a vault, register it
bzz start                        # start buzd (daemonizes; --foreground to stay attached)
bzz status                       # health check via the Unix socket
```

The Obsidian plugin (`plugin-obsidian/`, TypeScript) builds separately via its own `package.json` — it is not part of the Cargo workspace.

## Conventions

- Rust 2021 edition
- `tokio` for async
- `tracing` for logging (not `log` or `println!`)
- Error handling: `thiserror` for library crates, `anyhow` for binaries
- No `unwrap()` in library code; `expect()` only with a message explaining the invariant
- Tests alongside code (`#[cfg(test)]` modules), integration tests in `tests/`
- Format with `cargo fmt`; lint with `cargo clippy`

## Important technical details

- **External edit detection:** Uses `Automerge::Transaction::update_text` which internally runs Myers diff aligned to grapheme boundaries. No manual diff-to-ops code needed.
- **Ingestion gate:** Not all file changes should be broadcast. The gate classifies changes as Ingest (normal edit → broadcast), Absorb (tool operation → update locally, don't broadcast), or Defer (bulk operation in progress → wait, then batch-ingest final state).
- **Presence is ephemeral:** Cursor positions are broadcast on a separate QUIC stream, unreliable, not persisted in CRDT. 5-second timeout for stale cursors.
- **Write ordering:** Always write `.md` first, then `.buzzy/state/*.bin`. If daemon crashes between, the `.md` is canonical and buzd re-bootstraps from it.
- **`.buzzy/.gitignore` contains `*`:** This self-gitignores the CRDT state without requiring users to modify their top-level `.gitignore`.
