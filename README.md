# Buzzy

Real-time collaboration over files you own — across editors, across formats.

---

## What is Buzzy?

Buzzy is a daemon that makes any file on your disk collaborative in real-time — without changing the file's format, without requiring a specific editor, and without sending your work to a server.

Two people edit the same file simultaneously. Changes merge automatically via CRDT. No conflicts. No lock-outs. No central server deciding what's true. Your files stay in their native format on your machine.

## Components

| Name | What it is |
|------|-----------|
| `buzd` | Background daemon — manages CRDT state, file watching, sync |
| `bzz` | CLI — init, start, stop, share, pause, resume |

## Principles

1. **Your files, always.** Native format on your disk. Delete Buzzy and your files remain unchanged, complete, readable by any tool.
2. **Any editor.** Obsidian, VS Code, Neovim, Reaper — anything with a plugin. Or no plugin at all (edit in vim; the daemon detects changes and syncs).
3. **No server required.** LAN sync via mDNS + QUIC. Zero configuration. Optional relay for internet sync.
4. **Git coexistence.** Branch, merge, rebase — Buzzy detects git operations and stays out of the way.
5. **Format-extensible.** Text today. Code today. Structured formats (MIDI, video timelines, diagrams) via adapter trait — same protocol, same sync, same identity.

## How it works

```
┌────────────────────────────────────────────────┐
│  Your machine                                   │
│                                                │
│  ┌────────┐  ┌────────┐  ┌────────┐           │
│  │spec.md │  │main.rs │  │song.mid│           │
│  └───┬────┘  └───┬────┘  └───┬────┘           │
│      └───────────┼───────────┘                 │
│                  │                             │
│          ┌───────▼───────┐                     │
│          │     buzd      │                     │
│          └───────┬───────┘                     │
└──────────────────┼─────────────────────────────┘
                   │  P2P (QUIC over LAN)
┌──────────────────┼─────────────────────────────┐
│  Collaborator    │                             │
│          ┌───────▼───────┐                     │
│          │     buzd      │                     │
│          └───────┬───────┘                     │
│      ┌───────────┼───────────┐                 │
│  ┌───▼────┐  ┌───▼────┐  ┌───▼────┐           │
│  │spec.md │  │main.rs │  │song.mid│           │
│  └────────┘  └────────┘  └────────┘           │
└────────────────────────────────────────────────┘
```

- Both edit in whatever tool they prefer
- Changes appear in real-time (sub-500ms on LAN)
- Works offline — edits merge on reconnect (CRDT guarantees convergence)
- Files on disk are always the current, complete document in native format

## CLI (planned)

```bash
bzz init ~/vault              # initialize a vault for collaboration
bzz start                     # start buzd
bzz stop                      # stop buzd
bzz status                    # health check
bzz share spec.md --peer bob  # share a file
bzz pause                     # pause during git rebase / bulk ops
bzz resume                    # resume and sync final state
bzz peers                     # list peers on LAN
bzz log spec.md               # collaboration history
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Editor Plugins (thin JSON-RPC clients, ~200 lines each)     │
│  Obsidian • VS Code • Neovim • Any editor                   │
├─────────────────────────────────────────────────────────────┤
│  buzd (daemon)                                               │
│  ┌──────────────┐ ┌───────────────┐ ┌────────────────────┐  │
│  │ CRDT Engine  │ │ File Watcher  │ │ Network (mDNS+QUIC)│  │
│  │ (pluggable   │ │ + Ingestion   │ │ + Sync Protocol    │  │
│  │  via trait)  │ │   Gate        │ │                    │  │
│  └──────────────┘ └───────────────┘ └────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  Your files (native format, untouched)                       │
│  .buzzy/ (CRDT state — hidden, gitignored, deletable)        │
└─────────────────────────────────────────────────────────────┘
```

The CRDT engine is isolated behind a trait boundary. The daemon, networking, and editor plugins never depend on a specific CRDT library directly. The current implementation uses Automerge-rs; the trait allows swapping to Loro, yrs, or a custom engine without modifying any other crate.

## File layout

```
~/.buzd/                     # daemon config (global)
  identity.key               # Ed25519 keypair
  config.toml                # configuration
  peers.toml                 # known peers
  buzd.sock                  # Unix socket (runtime)

vault/                       # your files (untouched by buzd)
  spec.md
  main.rs
  .buzzy/                    # CRDT state (hidden, self-gitignored)
    .gitignore               # contains "*"
    index.json               # path ↔ UUID mapping
    state/<uuid>.bin          # per-document CRDT state
```

Delete `.buzzy/` → your files are unchanged. buzd rebuilds state from file content on next start.

## Key design decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| CRDT engine | Automerge-rs (pluggable) | Rust-native; ships `update_text` for external edit ingestion; Peritext marks for future comment anchoring |
| Transport | QUIC (quinn-rs) | Low latency; multiplexed; 0-RTT reconnect |
| Discovery | mDNS/DNS-SD | Zero-config LAN; no server needed |
| Identity | Ed25519 per-device | Decentralized; no account server |
| Editor protocol | JSON-RPC over Unix socket | Same pattern as LSP; thin plugins |
| Git coexistence | Ingestion gate | Detects `.git/index.lock`, `MERGE_HEAD`, rebase dirs; pauses during bulk ops |
| File ownership | `.buzzy/` separate from content | CRDT state is collaboration metadata, not your content; deletable without loss |

## Editor plugin contract

A Buzzy editor plugin is a JSON-RPC client that connects to `buzd.sock`. Minimal implementation is ~200 lines:

```
connect        → establish session
doc.open       → subscribe to a document
doc.edit       → send local changes (position-based insert/delete)
doc.remoteChange ← receive remote changes
doc.cursor     → report cursor position
doc.presence   ← receive peer cursors
```

The daemon handles all CRDT logic, sync, persistence, and network. The plugin handles UI only.

## Status

**Pre-alpha.** Architecture designed; implementation starting.

## Tech stack

| Component | Crate |
|-----------|-------|
| Async runtime | tokio |
| CRDT | automerge (pluggable via trait) |
| Transport | quinn (QUIC) |
| Discovery | mdns-sd |
| File watching | notify |
| Identity | ed25519-dalek |
| Hashing | sha2 |
| Serialization | serde, serde_json |
| CLI | clap |

## Documentation

| Doc | Description |
|-----|-------------|
| [docs/design.md](docs/design.md) | Architecture, permissions, external edit handling |
| [docs/mvp-lld.md](docs/mvp-lld.md) | Implementation spec (crate structure, protocols, code) |
| [docs/protocol-research.md](docs/protocol-research.md) | CRDT/sync/transport alternatives and decisions |
| [docs/crdt-engine-decision.md](docs/crdt-engine-decision.md) | Why Automerge-rs (and when to revisit) |
| [docs/research/](docs/research/) | Raw research: algorithms, protocols, formats, identity |

## License

MIT
