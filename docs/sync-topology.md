# Buzzy — Sync Topology

## Overview

Buzzy supports multiple sync modes simultaneously. The user doesn't choose a "mode" — the daemon uses whatever path is available, preferring the fastest. The guarantees (file ownership, data integrity) hold in all modes. Privacy level varies by deployment choice.

## Preference Cascade

The daemon transparently selects the best available sync path:

```
1. Direct (LAN peer available?)
   → mDNS discovery → QUIC connection → lowest latency, zero infra
   
2. Cloud (internet available, relay configured?)
   → Relay holds state → peers sync through it → works even if peers are never online simultaneously
   
3. Offline (no network at all)
   → Edit freely → CRDT state accumulates locally → merges when any path becomes available
```

These are not exclusive. A peer can sync some documents over LAN (with a nearby collaborator) and others via cloud relay (with a remote collaborator) simultaneously. The protocol is the same; only the transport differs.

## Sync Modes

### Mode 1: Direct (LAN / P2P)

```
┌──────────┐         QUIC (direct)         ┌──────────┐
│  buzd A  │◄──────────────────────────────►│  buzd B  │
│          │    mDNS discovery              │          │
│  (full   │    sub-50ms latency            │  (full   │
│   CRDT)  │    zero infrastructure         │   CRDT)  │
└──────────┘                                └──────────┘
```

- Both peers run full CRDT engine
- mDNS discovers peers on the same network
- Direct QUIC connection (no intermediary)
- Lowest latency; zero cost; no internet required
- Both peers hold complete document state

**When it's used:** two collaborators on the same WiFi/LAN (office, coworking, home network).

### Mode 2: Cloud Relay — Blind (E2E encrypted mailbox)

```
┌──────────┐                                ┌──────────┐
│  buzd A  │         ┌──────────┐           │  buzd B  │
│          │────────►│  Relay   │◄──────────│          │
│  (full   │  E2E    │          │  E2E      │  (full   │
│   CRDT)  │  cipher │ (stores  │  cipher   │   CRDT)  │
│          │◄────────│  bytes;  │──────────►│          │
└──────────┘         │  cannot  │           └──────────┘
                     │  read or │
                     │  merge)  │
                     └──────────┘
```

- Both peers run full CRDT engine locally
- Relay stores and forwards encrypted bytes
- Relay has NO CRDT logic, NO merge authority, CANNOT read content
- Peers may never be online simultaneously — relay holds messages until recipient connects
- Client does all merge work

**When it's used:** maximum privacy; users who don't trust any server with their content; compliance scenarios where data must not be readable by infrastructure.

**Trade-off:** both peers need the full CRDT engine (heavier clients); mobile/web clients must still run CRDT logic.

### Mode 3: Cloud — Trusted Server (CRDT in cloud)

```
┌──────────┐                                ┌──────────┐
│  Client A│         ┌──────────┐           │  Client B│
│          │────────►│  Cloud   │◄──────────│          │
│  (thin   │  ops    │  buzd    │  ops      │  (thin   │
│   or     │         │          │           │   or     │
│   full)  │◄────────│ (merges, │──────────►│   full)  │
└──────────┘  merged │  holds   │  merged   └──────────┘
              state  │  state)  │  state
                     └──────────┘
```

- Cloud runs a full `buzd` instance with CRDT engine
- Clients can be thin (send ops, receive merged state) OR full (run their own CRDT for verification)
- Cloud sees content in plaintext (or encrypted-at-rest, but decrypts to merge)
- Always available — collaborators don't need to be online simultaneously
- Cloud holds canonical CRDT state; clients sync against it

**When it's used:** teams that want thin mobile/web clients; self-hosted deployments where you trust your own infrastructure; managed Buzz Cloud (the commercial product).

**Trade-off:** cloud can read content. Mitigated by self-hosting or by client-side verification (see below).

### Mode 4: Cloud — Verifiable Server (CRDT in cloud + client verification)

```
┌──────────┐                                ┌──────────┐
│  Client A│         ┌──────────┐           │  Client B│
│          │────────►│  Cloud   │◄──────────│          │
│  (full   │  ops    │  buzd    │  ops      │  (full   │
│   CRDT)  │         │          │           │   CRDT)  │
│          │◄────────│ (merges) │──────────►│          │
│  verify: │  merged │          │  merged   │  verify: │
│  "does   │  state  └──────────┘  state    │  "does   │
│  this    │                                │  this    │
│  match   │                                │  match   │
│  what my │                                │  what my │
│  CRDT    │                                │  CRDT    │
│  would   │                                │  would   │
│  do?"    │                                │  do?"    │
└──────────┘                                └──────────┘
```

- Cloud runs CRDT and produces merge results
- Clients ALSO run CRDT locally and verify the cloud's output
- If verification fails: client flags the server as compromised; falls back to blind relay or direct P2P
- Best of both: thin-client UX benefit (cloud does heavy lifting) + trustless verification

**When it's used:** managed Buzz Cloud for security-conscious teams. The default for the commercial product.

**Trade-off:** clients still run CRDT engine (for verification), so the "thin client" benefit is partial — cloud handles availability and multi-peer fan-out, but clients aren't truly thin.

## Comparison

| Property | Direct (LAN) | Blind Relay | Metadata-Clear | Trusted Server | Verifiable Server |
|----------|-------------|-------------|----------------|----------------|-------------------|
| Cloud reads content | No | No | **No** (metadata only) | Yes | Yes (but verifiable) |
| Cloud can order/merge | N/A | No | **Yes** (via metadata) | Yes | Yes |
| Works offline | Yes | Yes (sync later) | Yes (sync later) | No (server required) | No (server required) |
| Peers must overlap online | Yes | No (mailbox) | No (server holds ordered ops) | No | No |
| Thin clients possible | No | No | No (client decrypts + renders) | Yes | Partial |
| Mobile/web friendly | No (full daemon) | No (full daemon) | No (full CRDT client) | Yes | Partial |
| Server can tamper | N/A | No (ciphertext) | Ordering only (content opaque) | Yes | Detectable |
| Self-hostable | N/A | Yes | Yes | Yes | Yes |
| Latency | Lowest (<50ms) | Medium (relay hop) | Medium | Medium | Medium |
| Infrastructure cost | Zero | Relay bandwidth | Server compute (ordering) | Server compute + storage | Server compute + storage |
| Metadata leakage | None | Timing only | Position + timing + patterns | Full content | Full content |

### Mode 5: Cloud — Metadata-Clear Merge (content-encrypted, structure-visible)

```
┌──────────┐                                    ┌──────────┐
│ Client A │         ┌───────────────┐          │ Client B │
│          │────────►│  Cloud buzd   │◄─────────│          │
│          │         │               │          │          │
│ Encrypts │  ops:   │ Sees:         │  ops:    │ Encrypts │
│ content  │  meta   │ • opIds       │  meta    │ content  │
│ locally  │  clear, │ • positions   │  clear,  │ locally  │
│          │  body   │ • causal deps │  body    │          │
│ Decrypts │  cipher │ • op types    │  cipher  │ Decrypts │
│ on recv  │         │               │          │ on recv  │
│          │◄────────│ Cannot see:   │─────────►│          │
└──────────┘  ordered│ • text content│  ordered └──────────┘
              ops    │ • what the doc│  ops
                     │   actually    │
                     │   says        │
                     └───────────────┘
```

- Clients encrypt operation **content** (the actual text being inserted) before sending
- Operation **metadata** (opId, parentId, position, type, causal links) remains in plaintext
- Server uses metadata to **order operations, resolve concurrent inserts, manage state vectors, fan out to peers**
- Server CANNOT read what was written — only where and when
- Clients decrypt operations on receive and reconstruct the document locally

**Why this works for CRDTs specifically:**

CRDT merge decisions depend on opIds and causal structure (metadata), not on the content of the text being inserted. Automerge's RGA resolves concurrent inserts by comparing opIds — a purely metadata operation. The server can compute the correct ordering without ever seeing what "hello world" says.

```
Wire operation format:
{
  op_id: (42, "alice"),           // ← plaintext (needed for ordering)
  parent_id: (41, "alice"),       // ← plaintext (needed for causality)
  type: "insert",                 // ← plaintext (needed for merge logic)
  position: 156,                  // ← plaintext (needed for placement)
  content: "aes-gcm:base64==...", // ← CIPHERTEXT (server cannot read)
  nonce: "...",                   // ← per-op encryption nonce
}
```

**What the server CAN do:**
- Order operations (resolve concurrent inserts by opId comparison)
- Detect missing operations (state vector management)
- Fan out to all peers (routing)
- Compact history (garbage collect by opId — doesn't need to read content)
- Hold state for offline peers (mailbox with ordering intelligence)

**What the server CANNOT do:**
- Read what was inserted (the characters, words, paragraphs)
- Reconstruct the document
- Search document content
- Validate markdown structure
- Serve a rendered view

**Metadata leakage (what the server learns):**
- Who edits which document, when (same as any E2E messaging system)
- Edit frequency and size (operation count, byte size of encrypted payload)
- Which position in the document was edited (character offset)
- Collaboration patterns (who responds to whose edits, timing)

For most use cases this is acceptable — same privacy level as Signal (content encrypted, envelope metadata visible). For adversarial scenarios (protecting a source, evading surveillance), blind relay or Tor is needed.

**When it's used:** managed Buzz Cloud where the operator wants to provide ordering/availability services without the legal/compliance burden of holding user content in plaintext. The server is "useful but not trusted with content."

**Trade-off:** clients still run full CRDT engine (decrypt + reconstruct). Position metadata reveals editing patterns. More complex than blind relay; less capable than trusted server (no thin clients, no server-side search).

**Implementation:**

```rust
// Client-side: encrypt content before sending to cloud
fn prepare_for_cloud(op: &CrdtOperation, doc_key: &SymmetricKey) -> WireOperation {
    WireOperation {
        op_id: op.op_id,                          // clear
        parent_id: op.parent_id,                  // clear
        op_type: op.op_type,                      // clear
        position: op.position,                    // clear
        content: aes_gcm_encrypt(doc_key, &op.content, &op.op_id_as_aad()),
        nonce: generate_nonce(),
    }
}

// Server-side: merge using metadata only
fn server_merge(ops: &[WireOperation]) -> MergeResult {
    // Compare op_ids for concurrent-insert ordering
    // Walk causal graph via parent_ids
    // Determine correct operation order
    // NEVER access .content field
    // Return ordered operation list for fan-out
}

// Client-side: decrypt after receiving ordered ops from cloud
fn receive_from_cloud(wire_op: &WireOperation, doc_key: &SymmetricKey) -> CrdtOperation {
    CrdtOperation {
        op_id: wire_op.op_id,
        parent_id: wire_op.parent_id,
        op_type: wire_op.op_type,
        position: wire_op.position,
        content: aes_gcm_decrypt(doc_key, &wire_op.content, &wire_op.op_id_as_aad()),
    }
}
```

---

## Architecture Implication

The daemon must support all modes through the same interface:

```rust
/// Transport abstraction — the daemon doesn't know which mode is active
pub trait SyncTransport: Send + Sync {
    /// Send a sync message to a peer (or to the cloud)
    async fn send(&self, peer: &PeerId, msg: &[u8]) -> Result<()>;
    
    /// Receive sync messages (from any peer or the cloud)
    async fn recv(&self) -> Result<(PeerId, Vec<u8>)>;
    
    /// Current connectivity status
    fn status(&self) -> TransportStatus;
}

enum TransportStatus {
    Direct { peer: PeerId, latency_ms: u32 },
    Relay { relay_addr: SocketAddr, encrypted: bool },
    CloudServer { endpoint: Url, verified: bool },
    Offline,
}
```

The CRDT engine, editor protocol, file watcher, and ingestion gate are identical regardless of which transport is active. Only the networking layer changes.

## The E2E + Server Merge Tension

The fundamental tension: if the server merges CRDT operations, it must understand them. If operations are encrypted, the server can't merge.

| Approach | How it resolves the tension | Status |
|----------|---------------------------|--------|
| **Blind relay** | Server doesn't merge — just stores+forwards ciphertext | Solved; simple |
| **Metadata-clear merge** | Server merges using operation metadata (opIds, positions, causality) while content remains encrypted | Feasible; CRDT-specific insight (merge logic depends on metadata, not content) |
| **Trusted server** | Accept that server sees content; protect with access control + audit | Solved; standard SaaS model |
| **Verifiable server** | Server sees content but clients verify correctness; tampering is detectable | Solved; adds client-side CRDT |
| **Full encrypted merge** | Server merges over fully encrypted operations using homomorphic or MPC techniques | Research-grade; not practical today |

**Buzzy's position:** support the first four. Let deployment context determine which is used:

- **LAN:** direct (no server at all)
- **Maximum privacy:** blind relay (nobody sees content, server is a dumb pipe)
- **Privacy + availability:** metadata-clear merge (server orders operations without reading content)
- **Open-source self-host:** trusted server (you trust your own infra)
- **Managed Buzz Cloud:** verifiable server (we run it; you verify)

## Deployment Spectrum

```
Maximum privacy ◄─────────────────────────────────────────────► Maximum convenience
                                                               
Blind relay     Metadata-clear     Verifiable server    Trusted server
(full E2E;      (content E2E;      (cloud merges;       (your infra;
 dumb pipe;      server orders      clients verify;      full thin-client
 heavy clients)  by metadata)       medium clients)      support)

                      ▲
                      │
               Default for Buzz Cloud
               (privacy + availability sweet spot)
```

Users/teams choose their position on this spectrum via configuration — and different files within the same vault can use different modes.

### Per-Path Topology Rules

The daemon treats each document independently (separate CRDT state, separate UUID, separate sync session). Topology selection is a per-document routing decision, not a global setting.

Configuration uses glob-pattern rules, most specific match wins:

```toml
# vault/.buzzy/config.toml

[sync]
# Default mode for the vault (applies when no rule matches)
# Principle: privacy by default. metadata-clear is the default because
# content is encrypted (server cannot read it) while still providing
# ordering and availability. Users opt DOWN to "blind" for maximum privacy,
# or opt UP to "trusted" for thin-client convenience.
default_mode = "metadata-clear"

# Default cloud endpoint
endpoint = "https://relay.buzz.dev"

# Preference order for transport paths
prefer = ["direct", "cloud", "offline"]

# Per-path overrides (most specific glob wins)
[[sync.rules]]
path = "journal/**"
mode = "blind"                    # private thoughts — maximum privacy; server is dumb pipe

[[sync.rules]]
path = "team/specs/**"
mode = "metadata-clear"           # team docs — server orders, can't read content

[[sync.rules]]
path = "public/**"
mode = "trusted"                  # published content — server can render previews, enable web viewer

[[sync.rules]]
path = "contracts/**"
mode = "blind"
timestamp = true                  # legal docs — blind sync + OpenTimestamps notarisation

[[sync.rules]]
path = "scratch.md"
mode = "direct"                   # never leaves the LAN — no cloud sync at all

[[sync.rules]]
path = "design/*.puml"
mode = "metadata-clear"
share = false                     # synced to cloud for backup, but not shared with others
```

### How the daemon resolves topology per document

```
1. Document opened (by editor or watcher)
2. Resolve vault-relative path
3. Match against sync.rules (most specific glob wins; fall back to default_mode)
4. Select SyncTransport implementation for that mode
5. Establish sync session with the appropriate endpoint
6. Document syncs via its assigned topology independently of all other documents
```

### Examples of mixed topology in one vault

```
vault/
├── journal/                     → blind relay (private)
│   ├── 2026-07-08.md
│   └── reflections.md
├── team/
│   └── specs/                   → metadata-clear (team collab, server can't read)
│       ├── api-v2.md
│       └── architecture.md
├── public/                      → trusted server (publishable, server renders)
│   └── blog-draft.md
├── contracts/                   → blind relay + timestamps (legal)
│   └── nda-acme.md
├── scratch.md                   → direct only (LAN, never touches cloud)
└── .buzzy/
    └── config.toml              (contains the rules above)
```

All of these sync simultaneously through different paths. The daemon manages multiple `SyncTransport` instances in parallel — one per active topology mode. A document's topology can be changed at any time by updating the config; the daemon re-routes on next sync cycle.

### Sharing across topology boundaries

When Alice shares a document with Bob, the topology rule applies to Alice's copy independently of Bob's:

- Alice has `contracts/nda.md` set to `blind` mode
- She shares it with Bob
- Bob receives it and can configure his own topology for his copy (e.g., `metadata-clear`)
- The relay/cloud handles the intersection: if Alice's side is `blind`, the server stores her ops as ciphertext; Bob's daemon decrypts on receive regardless of what mode Bob uses locally

The **most private** participant's topology governs what the server sees for their operations. Each peer's topology is independent — you control your own privacy level without imposing it on collaborators.

### Global daemon config (transport-level defaults)

```toml
# ~/.buzd/config.toml (global, applies to all vaults)

[network]
mdns_enabled = true               # enable LAN discovery
quic_port = 4433
quic_bind = "0.0.0.0"

[cloud]
endpoint = "https://relay.buzz.dev"
fallback_endpoint = "https://relay-eu.buzz.dev"

[defaults]
# Privacy by default: content always encrypted unless user explicitly opts into "trusted"
mode = "metadata-clear"           # global default (overridden by per-vault config)
```

Per-vault `.buzzy/config.toml` overrides global defaults. Per-path rules in the vault config override the vault default. Most specific wins.

## What This Means for the MVP

The MVP ships **direct mode only** (LAN, P2P). This is the simplest path and validates the core CRDT + file-watching + editor-plugin architecture.

Post-MVP adds:
1. **Blind relay** (E2E encrypted store-and-forward) — for internet sync with maximum privacy
2. **Metadata-clear merge** (content-encrypted, server orders by metadata) — privacy + availability sweet spot; default for managed Buzz Cloud
3. **Trusted server** (cloud buzd with full access) — for teams who self-host and want thin clients
4. **Verifiable server** — for security-conscious teams on managed cloud

The trait boundary (`SyncTransport`) ensures all modes use the same CRDT engine, same editor protocol, same file format. Only the pipe changes.

## Decision Record

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Support five sync modes (direct, blind relay, metadata-clear, trusted server, verifiable server) | Different users have different trust requirements; forcing one model excludes markets |
| 2 | Direct (LAN) is the MVP | Simplest; validates core architecture without networking complexity |
| 3 | Metadata-clear merge is the default for managed cloud | Best privacy/convenience balance — server provides ordering+availability without reading content |
| 4 | Transport is abstracted behind a trait | Daemon code is identical regardless of sync mode; only the transport layer changes |
| 5 | Files on disk are canonical in ALL modes | Even in trusted-server mode, the local `.md` file is truth; cloud state is a convenience copy |
| 6 | Preference cascade is automatic | User doesn't manually select mode; daemon picks the best available path |
| 7 | Encryption is per-document | Some documents may use blind relay (sensitive); others may use metadata-clear (convenience); mixed within one vault is fine |
| 8 | CRDT merge depends on metadata, not content | This is the key insight that makes metadata-clear mode feasible — Automerge's RGA resolves concurrent inserts by opId comparison (metadata), so the server can order operations without reading the text |
