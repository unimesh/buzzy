# Sync Protocols and Wire Formats for Real-Time Collaboration and Local-First Software

Technical comparison for the buzzy design document. Covers Automerge and Yjs sync mechanics, CRDT wire formats, transport options (WebSocket, WebRTC, QUIC/Iroh, libp2p, Hypercore), existing frameworks (automerge-repo, y-sweet, Liveblocks, PartyKit, Iroh, Syncthing, ElectricSQL), and encoding formats. Ends with a recommendations matrix mapped to four deployment scenarios and a list of non-obvious caveats worth flagging in the design.

---

## 1. Executive summary table

| Name | Category | Language(s) | Maturity | Presence built-in? | Primary use case |
|---|---|---|---|---|---|
| **Automerge sync protocol** | Sync | Rust core + TS/WASM, Swift, Python bindings | Production (Automerge 2, spec published) | No (ephemeral messages added in automerge-repo) | General-purpose CRDT sync over any byte stream |
| **Automerge columnar format** | Encoding | Rust | Production; spec at `automerge.org/automerge-binary-format-spec/` | N/A | On-disk + wire format for Automerge |
| **Yjs sync protocol (y-protocols)** | Sync | TS/JS (`lib0`); Yrs mirrors in Rust | Production (spec = `PROTOCOL.md`) | Separate module (`awareness`) multiplexed on same channel | Yjs doc replication over any byte stream |
| **Yjs lib0 binary format** | Encoding | JS + Rust (Yrs) v1/v2 compat | Production | N/A | Yjs updates, snapshots, state vectors |
| **Loro** | Encoding + CRDT lib | Rust core + WASM/Swift | v1.x stable | Separate | Fugue-based text + rich structures |
| **Diamond Types** | Encoding + CRDT lib (text-only) | Rust + WASM | Pre-1.0; text-only | Separate | High-throughput plain-text replay (Eg-walker) |
| **Y-CRDT (Yrs)** | Encoding + CRDT lib | Rust + C ABI + JS/Py/Rb/.NET/Swift/Kotlin | Production (lib0 v1/v2 wire-compat) | Awareness | Yjs-compatible cross-language sync |
| **WebSocket (y-websocket)** | Transport | JS (Node `ws`) | Production | In-protocol (multiplexed with sync) | Client-server Yjs sync |
| **WebRTC data channel (y-webrtc)** | Transport | JS (`simple-peer`) | Production | In-protocol | P2P mesh with signaling |
| **QUIC / Iroh** | Transport + stack | Rust + FFI (Swift/Kotlin/Py/JS/C) | v1.0.1 (2026) | Not in transport; via iroh-gossip / docs | Content-addressed P2P with NAT traversal |
| **libp2p** | Transport | Go/Rust/JS | Production; **no official Automerge adapter** | No | General P2P (would-be adapter for Automerge) |
| **Hypercore / Hyperswarm** | Transport | JS (Holepunch) | Production | No | Append-only log replication, DHT |
| **Automerge-repo** | Framework | TS (98.8%) + WASM Rust core | Production | Yes — `Presence` primitive | Doc-graph + pluggable network/storage |
| **y-sweet** | Framework/service | Rust core + TS SDK; MIT | v0.9.x (pre-1.0) | Yjs awareness | Self-hostable Yjs server with S3 |
| **Liveblocks** | Service (closed) | Proprietary server; open TS SDKs | Production; commercial | Yes — first-class + Yjs awareness bridge | Managed real-time app backend |
| **PartyKit / y-partykit** | Framework/service | TS on Cloudflare Workers/DO | Production (Cloudflare-acquired Apr 2024) | Room broadcast + Yjs awareness | Per-doc Durable Object rooms |
| **Iroh (docs + blobs + gossip)** | Full stack | Rust + FFI | v1.0.1 | Via iroh-gossip live events | P2P KV-CRDT + verified blob sync |
| **Syncthing BEP v1** | Sync (files) | Go | Production | No | Block-based file sync, not CRDT ops |
| **ElectricSQL** | Framework | Elixir + TS | v1.0 (Mar 2025, read-path only) | No | Postgres → client read cache (not CRDT) |
| **Protocol Buffers / FlatBuffers / MsgPack / CBOR** | Encoding | Many | Production | N/A | Generic serialization; **not adopted by collab systems** |

---

## 2. Per-item deep dives

### 2.1 Automerge sync protocol

- **Solution vs component:** Component — a stateless request/response protocol per peer, run on top of any bytestream.
- **Language support:** Canonical implementation in Rust (`rust/automerge/src/sync.rs`) with JS/WASM, Python, and Swift bindings.
- **Offline/reconnect:** Handled by design. `SyncState` persists `shared_heads` across sessions; the rest is transient. On reconnect, both sides regenerate messages from their heads and bloom filters — no separate re-handshake protocol.
- **Bandwidth efficiency:** Excellent. The Bloom-filter "have" set means senders don't retransmit changes the peer already has, and the change chunks themselves are columnar+RLE compressed.
- **Latency:** Low — a single round trip when there are no divergent hashes; more when the graph forks.
- **Presence/awareness:** Not part of the protocol. Automerge-repo adds a `Presence` primitive on top.

`Message` struct:

```rust
pub struct Message {
    pub heads:   Vec<ChangeHash>,   // sender's frontier (32-byte SHA-256)
    pub need:    Vec<ChangeHash>,   // hashes sender explicitly requests
    pub have:    Vec<Have>,         // Bloom summary of what sender already has
    pub changes: ChunkList,         // change chunks for recipient to apply
    pub flags:   Option<MessageFlags>,
    pub version: MessageVersion,
}
pub struct Have { last_sync: Vec<ChangeHash>, bloom: BloomFilter }
```

Wire byte layout:

1. Version byte: `MESSAGE_TYPE_SYNC = 0x42` (V1) or `MESSAGE_TYPE_SYNC_V2 = 0x43`.
2. `heads`: `uLEB128(count)` then `count × 32` bytes.
3. `need`: `uLEB128(count)` then `count × 32` bytes.
4. `have`: `uLEB128(count)` then per entry `last_sync` array + bloom bytes.
5. `changes`: `uLEB128(count)` then per chunk `uLEB128(len)` + bytes.
6. Trailing V2 flags: `SYNC_RESET`, `READ_ONLY`, `SUPPORTS_SYNC_RESET`.

**Bloom filter:** default 10 bits/entry, 7 probes, ~1% false positive. Parameters ride the wire (backward-compatible tuning). ChangeHash is the hash input directly (SHA-256 is already random). Probe positions derive from three little-endian u32 words via double-hashing.

**Send-set algorithm:** walk reachable beyond `⋃ have.last_sync`; drop hashes the peer's bloom marks present; add forward-closure for causality; prepend explicit `need`. If send set > ~⅓ of graph and peer supports V2, substitute a full `save()` document (compaction shortcut).

Sources: `github.com/automerge/automerge`, `automerge.org/automerge-binary-format-spec/`.

### 2.2 Yjs sync protocol (y-protocols)

- **Solution vs component:** Component — a two-message framed protocol multiplexed with awareness under a composite ID.
- **Language support:** JS canonical (`y-protocols`, `lib0`); Rust via Yrs preserves byte-level compat.
- **Offline/reconnect:** Handled — Yjs updates are commutative + idempotent; reconnect re-runs the two-message handshake.
- **Bandwidth efficiency:** Excellent — state-vector-based diff.
- **Latency:** Low — client-server converges in one round trip.
- **Presence/awareness:** Separate module (`y-protocols/awareness`), same channel via composite framing.

Message type IDs:
```
messageYjsSyncStep1 = 0
messageYjsSyncStep2 = 1
messageYjsUpdate    = 2
```

Wire:
- SyncStep1 = `varUint(0) • varBuffer(stateVector)`
- SyncStep2 = `varUint(1) • varBuffer(documentUpdate)`
- Update = `varUint(2) • varBuffer(documentUpdate)`

State vector encoding: `varUint(numClients) • (varUint(clientID) • varUint(clock))*`. Each clock is the next-expected Lamport clock.

**Composite framing:** outer `varUint(topLevelType)` — `0 = SyncProtocol`, `1 = AwarenessProtocol`. Read-only enforcement is trivial: `00 00 …` is safe (SyncStep1); `00 01 …` and `00 02 …` are writes.

**Awareness protocol:**
```
awarenessUpdate := varUint(numEntries) •
                   (varUint(clientID) • varUint(clock) • varString(JSON.stringify(state)))*
```
`outdatedTimeout = 30000ms`, check interval `3000ms`. `state = null` marks offline.

**Note:** awareness values pass through `JSON.stringify` — no `Uint8Array`, `BigInt`, `undefined`, or `NaN`.

**lib0 primitives:**
- `writeVarUint`: LEB128, LSB-first.
- `writeVarInt`: **non-standard** — first byte's bit 7 is a sign flag (not zig-zag).
- `writeVarUint8Array = varUint(len) • bytes`.
- Floats in `writeAny` are **big-endian**.

Sources: `github.com/yjs/y-protocols/blob/master/PROTOCOL.md`, `github.com/dmonad/lib0`.

### 2.3 CRDT wire formats

**Vendor-neutral standard: none.** Datatracker search for "CRDT" returns zero drafts. Matrix MSC4033/MSC4059 stalled. Interop remains unsolved.

**Automerge columnar** — Column-spec is a 32-bit uLEB bitfield: low 3 bits = column type (Group, Actor, uLEB, Delta, Boolean, String, ValueMetadata, Value), bit 3 = DEFLATE (document chunks only). RLE uses signed-LEB `(length, value)` pairs. Container framing: `magic 85 6F 4A 83 • SHA256[0..4] checksum • chunk_type:u8 • uLEB(len) • contents`. Change hash = `SHA256(0x01 || uLEB(len) || contents)`.

**Yjs / lib0** — v1/v2 formats are de-facto spec (no separate written spec beyond `PROTOCOL.md`). Yrs guarantees byte compatibility.

**Loro** — Rust core + WASM/Swift. 22-byte header (`loro` magic + 16-byte checksum + u16 mode). Outdated modes = columnar RLE via `serde_columnar` with MD5; Fast modes = KV-of-blobs with XXH32. Uses Fugue algorithm.

**Diamond Types** — 8-byte magic `DMNDTYPS` + varint version. Uses protobuf-style varints. Text-only in mainline; JSON/list/map on `more_types` branch. ~5,000× faster than Automerge on Kleppmann's 260k-edit trace. Author's own recommendation: *"If you're building a document based collaborative application today, you should use Yjs."*

**Yrs** — Byte-level compat with JS Yjs in v1 and v2. Bindings: Rust, C ABI, WASM, Python (`pycrdt`; `ypy` archived Apr 2025), Ruby, .NET, Swift, Kotlin, R.

**Bandwidth (Kevin Jahns's B4, 260k ops):**
- Automerge 2.x: **129,116 bytes** (~1.23× final text)
- Yjs: **159,929 bytes** (~1.53×)
- Loro: **258,228 bytes**
- Automerge 0.14.1 (pre-columnar): 84 MB — 650× regression the columnar rewrite fixed.

Diamond Types on Kleppmann's automerge-perf trace: raw JSON 16 MB → gzipped 904 KB → DT full 281 KB → **DT patches-only 23 KB**.

### 2.4 Transport protocols

**y-websocket** — Node uses `ws`. Exponential-backoff reconnect (`maxBackoffTime = 2500ms`). Cross-tab via BroadcastChannel + localStorage fallback. Awareness rides same socket. Bundled in-memory server is not for production; alternatives: `@y/hub`, `hocuspocus`, `y-sweet`, `yrs-warp`. No NAT traversal.

**y-webrtc** — `simple-peer` for `RTCPeerConnection`. Full-mesh within a room up to `maxConns = 20 + rand(15)`. Signaling via WebSocket to multiple public relays (`wss://signaling.yjs.dev`, etc.); minimal signaling server ships as `bin/server.js`. Room-scoped optional symmetric encryption of signaling traffic; data channel is DTLS-encrypted at browser level. STUN/TURN caller-supplied.

**QUIC / Iroh** — Rust core, FFI bindings via separate `iroh-ffi` (releases paused Feb 2025 pending redesign). Multiple protocols multiplex via ALPN through one QUIC connection. Endpoints keyed by public key (`EndpointId`), never IP. NAT traversal uses `n0_nat_traversal` QUIC extension (based on IETF `draft-seemann-quic-nat-traversal`) + QUIC Address Discovery. Fallback: **iroh-relay** (architectural equivalent of Tailscale's DERP). Peer discovery: Pkarr DNS at `dns.iroh.link`, optional Mainline DHT.

**iroh-blobs:** BLAKE3 content-addressed. Verified streaming via BAO outboard tree hashes — receivers verify each 1 KiB chunk, enabling verified range fetches and resumable transfers with ~6% metadata overhead.

**iroh-gossip:** HyParView (peer sampling) + Plumtree (epidemic broadcast) on QUIC. Split into `proto` (state machine) + `net` (transport). 32-byte `TopicId`.

**iroh-docs:** Multi-writer KV keyed by `(NamespaceId, AuthorId, key)`; entries hold BLAKE3 hash + size + signed timestamp only. **Conflict resolution: LWW on timestamp** (not Lamport clock). Convergence via range-based set reconciliation (Meyer 2022). Live updates via iroh-gossip. Persistence: `redb`.

**libp2p + Automerge** — **no official adapter.** `automerge-repo` ships WebSocket, MessageChannel, BroadcastChannel; PeerJS/WebRTC lives in a separate repo. Anyone wanting Automerge over libp2p writes a custom `NetworkAdapter`.

**Hypercore / Hyperswarm** — JS. Signed-Merkle append-only log; encrypted Noise stream with Protomux multiplexing. Wire opcodes: `block`, `hash`, `seek`, `upgrade`. Sparse fetch via `core.download()`. Hyperswarm joins peers by 32-byte topic via HyperDHT with UDP hole punching. Ed25519 keypairs. Historical CRDT use: Cabal via `kappa-core`, Beaker via `hyper://`.

**Willow protocol** — separate spec (`willowprotocol.org`), NLnet-funded. Custom variable-width tag scheme (not CBOR). Iroh sponsors; `iroh-willow` integration exists.

### 2.5 Sync frameworks and services

**Automerge-repo** — TS (98.8%) wrapping Automerge WASM Rust core. Also a Rust port `automerge-repo-rs` (less mature) and Swift wrapper. Three official network adapters (WebSocket, MessageChannel, BroadcastChannel — the last "likely only useful for experimentation"). Two storage adapters (IndexedDB, nodefs). **Presence is first-class** via `Presence` primitive with heartbeats. No built-in auth — delegated via `sharePolicy`. Sync topology is star (via `automerge-repo-sync-server`) but supports P2P adapters. Capability-based auth work in `automerge/beelay` isn't integrated.

**y-sweet (Jamsocket)** — Rust core + TS SDKs (`@y-sweet/sdk`, `-client`, `-react`), Python SDK. MIT. Three storage backends: S3, local FS, in-memory (default). Tiered hot/cold is emergent (per-doc process = hot; S3 = cold) but not marketed as such. Auth via signed doc-scoped tokens (`manager.getOrCreateDocAndToken(docId)`). Presence = Yjs awareness. Latest v0.9.1 (Sep 2025), pre-1.0.

**Liveblocks** — Closed server; open-source client SDKs. Proprietary WebSocket protocol on Cloudflare Workers; wire format unpublished. Two storage models: their own `LiveObject`/`LiveList`/`LiveMap` CRDT and `@liveblocks/yjs` for Yjs. In Yjs path, awareness bridged into Liveblocks Presence (`room.getPresence().__yjs`). Threads/Comments first-class. Pricing: Free (3,000 collab-minutes cap), Pro $30/mo, Team $600+/mo, Enterprise custom. HIPAA BAA $350/mo on Team.

**PartyKit / y-partykit** — TS on Cloudflare Workers/DO. Acquired by Cloudflare 5 Apr 2024. Each party = one DO, deterministic ID routing. **Hibernation** via Workers WebSocket Hibernation API raises per-room ceiling from ~100 to ~32k concurrent. After hibernation, `constructor` + `onStart` re-run; handlers added in `onConnect` are lost — use `onMessage`/`onClose`. Storage via DO transactional KV; R2 available. WebSocket only, no WebRTC. `y-partykit` wraps Yjs with two persistence modes: **snapshot** (merged on last-client-leave) or **history** (default 10 MB via `maxBytes`/`maxUpdates`). Debounced `callback.handler(yDoc)` for external DB sync.

**Iroh (n0)** — Full stack. `iroh-ffi` releases paused Feb 2025. Browser alpha early 2025. iroh-blobs modularized out of core in v0.90 (Jul 2025); v0.95 Oct 2025. v1.0.1 (2026). Strategy "Custom Protocols For All!" — networking is the core, blobs/docs/gossip are peripheral crates.

**Syncthing BEP v1** — Go. TCP (also QUIC + Relay pool) with TLS 1.3+ mutual auth. Frames are **Protocol Buffers** (proto3), big-endian: `[u16 header_len][Header pb][u32 msg_len][Message pb]`. Optional LZ4. Max message 500 MB. Device ID = 32-byte SHA-256 of self-signed X.509 (**ECDSA P-384**, not Ed25519) — base32-encoded with check digits. Blocks 128 KiB–16 MiB (adaptive to keep <2000 blocks/file), each SHA-256 hashed. Messages: `CLUSTER_CONFIG, INDEX, INDEX_UPDATE, REQUEST, RESPONSE, DOWNLOAD_PROGRESS, PING (90s idle), CLOSE`. Conflict model = per-file version vectors. **CRDT applicability is poor** — content-addressed block transfer, no concurrent-edit merging within a block.

**ElectricSQL** — Pivoted. Now read-path sync engine for Postgres via Shapes (v1.0 Mar 2025). No client CRDT merge. Server-authoritative. Transport: low-level HTTP long-poll/streaming. Server in Elixir/BEAM; TS + Elixir clients.

**Adjacent tools** — TinyBase (native `MergeableStore` CRDT + optional Yjs/Automerge/CR-SQLite bridge); Replicache (server-authoritative rebase, maintenance mode, successor is Zero); PowerSync (change-stream server-auth); Rocicorp Zero (query-driven, ZQL over Postgres replica).

### 2.6 Encoding formats

- **Protocol Buffers** — dominant in RPC, **not adopted as an op wire format** by any major collab engine (Google Docs, Firebase, Fluid, ShareDB all avoid it for ops). Diamond Types borrows protobuf's varint primitive without the framing. Wire overhead vs custom binary typically 20–40% (tag+wire-type byte per field). *Sync framing* adopter: Syncthing BEP v1 uses proto3 for headers + messages.
- **FlatBuffers** — zero-copy vtable-indexed. **No known adopter** in CRDT space. Streaming op deltas benefit more from columnar RLE than vtable indirection.
- **MessagePack** — compact JSON-shape binary. Widely used in Redis scripting, Fluentd. **Not adopted by any major CRDT/collab system.** Schemaless.
- **CBOR** — RFC 8949. Prominent in COSE, WebAuthn. **Willow explicitly rejects CBOR** for its own tag scheme. No mainstream CRDT uses CBOR as primary op format.
- **Custom binary formats dominate** — Automerge columnar, Yjs lib0, Loro columnar/KV, Diamond Types.

**JSON persists** in Firebase Realtime Database, ShareDB, and Fluid Framework Routerlicious (`ISequencedDocumentMessage` with socket.io transport). JSON envelopes typically 150–400 bytes/op — 5–10× Yjs's 27 B avg. Fluid's stream: Alfred → Kafka → Deli → Kafka → Scriptorium → MongoDB, with Redis fan-out; presence is a separate `ISignalMessage` channel. **Microsoft Loop uses Fluid Framework.**

---

## 3. Recommendations matrix

### (a) Browser-only collaborative editor
**Stack:** Yjs + y-websocket + y-partykit (Cloudflare) *or* y-sweet (self-host).
**Why:** Smallest client bundle among mature CRDT libs; awareness protocol built-in for cursors/selections; y-websocket reconnect + status UI is clean; per-doc rooms with predictable cost. WebRTC (y-webrtc) is only worth adding for opportunistic P2P — signaling still required, full-mesh caps ~35 peers/room.

### (b) Native desktop with offline-first
**Stack:** Automerge (Rust core) + automerge-repo + WebSocket + IndexedDB/filesystem storage.
**Why:** Documented binary format; Rust-native core; `Presence` primitive covers cursors without separate awareness. Bloom-filter sync makes reconnects cheap after long offline. The ">⅓ of graph → send everything" heuristic prevents catch-up storms.
**Alternative:** Yjs + Yrs (Rust) if the app also has web UI — Yrs preserves byte-level compat so desktop and web share one channel.

### (c) P2P mobile without central server
**Stack:** Iroh + iroh-docs (or Iroh QUIC + Automerge as an ALPN protocol).
**Why:** Solves NAT traversal (QUIC hole-punching + relay fallback) and stable identity (public-key EndpointIds survive IP changes). Swift/Kotlin FFI exists (but `iroh-ffi` releases paused Feb 2025 — verify status before committing). iroh-blobs' BLAKE3 verified range fetches minimize battery/data cost.
**Alternative:** y-webrtc with self-hosted signaling if browser-first and full-mesh limits are acceptable.

### (d) Enterprise with audit/compliance
**Stack:** Liveblocks (managed, HIPAA BAA) *or* Automerge + custom sync server on managed K8s.
**Why:** Enterprise buyers want signed BAA, SOC 2, single-tenant. Liveblocks provides these; Yjs bridge means no proprietary CRDT lock-in. For on-prem: Automerge + `automerge-repo-sync-server` fork. Automerge's documented spec is auditable in a way Yjs (spec-by-implementation) is not. Columnar format's DEFLATE gives cheap at-rest compression.
**Avoid:** Rolling a proprietary CRDT format — bandwidth savings marginal, audit surface expanded. Avoid ElectricSQL if the design requires client-side CRDT merge (current product is server-authoritative only).

---

## 4. Key caveats

- **Automerge-Repo has no native WebRTC adapter** in monorepo; PeerJS variant is separate.
- **Iroh's `iroh-ffi`** paused releases Feb 2025; non-Rust bindings should be considered experimental until this ships.
- **Yjs awareness values pass through `JSON.stringify`** — typed data (`Uint8Array`, `BigInt`, `undefined`, `NaN`) is lost.
- **Automerge's source has `ChunkType::Bundle = 3`** not documented in the public binary spec — forward-compat consideration for third-party parsers.
- **Diamond Types is text-only in mainline**; JSON/list/map is on `more_types` branch, no release.
- **Syncthing's device ID is ECDSA P-384**, not Ed25519 as some writeups state.
- **No vendor-neutral CRDT wire format exists.** Automerge and Yjs are the only mature choices for cross-language sync (via Yrs); interop between them is not on any roadmap.
- **PB/FlatBuffers/MsgPack/CBOR are not used by any major CRDT** as op wire format — columnar RLE dominates on bandwidth grounds.
