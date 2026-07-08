# Buzzy — CRDT Engine Decision: Automerge-rs

## Decision

Automerge-rs (`automerge` crate, v0.10.x) is buzzy's CRDT engine. This document records the reasoning, alternatives evaluated, trade-offs accepted, and conditions under which the decision would be revisited.

---

## Context

buzzy is a daemon-based collaborative editing protocol. The CRDT engine is the foundation — it determines how concurrent edits merge, what metadata is stored per character, how external edits are ingested, and what the performance ceiling looks like. The choice is load-bearing for every other architectural decision.

The engine must satisfy six requirements simultaneously:

1. **Rust-native** — the daemon is Rust; FFI bridges add complexity, latency, and maintenance burden
2. **External edit ingestion** — when a user edits the `.md` in vim/sed/any tool, the daemon must translate the file diff into CRDT operations with a single API call
3. **Comment anchoring** (post-MVP) — inline comments must attach to text ranges that survive concurrent edits without orphaning
4. **Operation history** — AI features (operation-stream intelligence, per-user edit attribution, selective revert) require full edit history
5. **Published binary format** — the sidecar file must be documented, forward-compatible, and readable by third-party tools
6. **Sync protocol** — peers must exchange only missing changes efficiently; the protocol should handle offline/reconnect gracefully

---

## Candidates Evaluated

### Automerge-rs (RGA + Peritext)

- **Algorithm:** Replicated Growable Array. Each character is an immutable node in a causal tree; inserts name a parent (left neighbour at insertion time) with a globally-unique opId `(counter, actorId)`. Concurrent inserts under the same parent ordered by opId.
- **Implementation:** Rust core (`rust/automerge/`); JS/WASM, Swift, Python, C, Java bindings all wrap the Rust library via FFI.
- **Version:** 0.10.0 (stable); formerly marketed as "Automerge 3.x" in the JS ecosystem.
- **Maintainers:** Alex Good, Orion Henry, with Ink & Switch collaboration. Full-time development.
- **Stars/activity:** 6.4k GitHub stars; active commits through 2026.

### yrs (YATA — Rust port of Yjs)

- **Algorithm:** YATA (Yet Another Transformation Approach). Items linked by `origin` + `rightOrigin`; concurrent inserts ordered by client ID. Run-length encoded Items for memory efficiency.
- **Implementation:** Rust port of Yjs; aims for byte-level wire compatibility with the JavaScript reference.
- **Position:** Compatibility implementation, not the reference. Yjs in JS is the reference; yrs tracks it.
- **Maintainers:** Bartosz Sypytkowski + community. Part of the y-crdt GitHub org.

### Diamond Types (Eg-walker)

- **Algorithm:** Event-graph walker. Stores an append-only edit log + causal links; transforms events on merge rather than maintaining permanent CRDT metadata. Steady-state memory proportional to document size, not edit count.
- **Implementation:** Rust. `diamond-types` crate. Single-maintainer (Joseph Gentle).
- **Status:** Pre-1.0. README acknowledges cargo package is "quite out of date, both in terms of API and performance."

### Fugue (via Loro)

- **Algorithm:** Fugue (Weidner & Kleppmann, IEEE TPDS 2025). Tree-structured insert with provably maximal non-interleaving — the only algorithm with a formal proof that concurrent character runs never interleave.
- **Implementation available:** Loro (`loro-dev/loro`) — Rust CRDT library using Fugue internally. v1.x stable, MIT licensed.
- **Direct port option:** The Fugue algorithm is ~500-800 lines of core logic; a direct Rust port is feasible in 2-4 weeks but provides only the merge algorithm, not the surrounding infrastructure.

### Fugue (raw TypeScript reference)

- **Implementation:** TypeScript, part of Weidner's Collabs library.
- **Status:** Research code. No Rust implementation. No wire format. No sync protocol.

---

## Comparison Matrix

| Requirement | Automerge-rs | yrs | Diamond Types | Loro (Fugue) | Fugue (raw port) |
|-------------|-------------|-----|---------------|-------------|-----------------|
| Rust-native | **Yes** (IS the reference) | Yes (port, not reference) | Yes | Yes | Would be after port |
| `update_text` (diff → ops) | **Ships in `text_diff.rs`** | No equivalent | No | No | No |
| Peritext marks (comments) | **Built-in** | DIY via RelativePosition | No | Own marks system (not Peritext) | No |
| Full operation history | **Default** (all changes retained) | No (trades history for perf) | Event graph (full) | Yes | No |
| Published binary spec | **Yes** (forward-compat guaranteed) | No (spec-by-implementation) | Informal BINARY.md | 22-byte header + XXH32, not formally spec'd | No |
| Sync protocol | **Bloom-filter delta** (built-in) | y-protocols (built-in) | None | None | None |
| Non-interleaving quality | Good (not provably maximal) | Good (not provably maximal) | YATA-family | **Provably maximal** | **Provably maximal** |
| Memory efficiency | O(ops), compaction mitigates | O(items), RLE-compressed | **O(document) steady-state** | O(ops) | O(items) |
| Editor bindings ecosystem | Moderate (ProseMirror, CM6) | **Largest** (PM, Tiptap, Quill, CM, Monaco, Slate) | Minimal (WASM) | Minimal | None |
| Per-user undo | Caller-managed (via history) | **Y.UndoManager** (first-class) | Not addressed | Yes | No |
| Production maturity | **High** (v0.10, Isabelle proofs) | High | Pre-1.0, single maintainer | v1.x stable but younger | Research code |

---

## The Decisive Factor: `Transaction::update_text`

buzzy's defining architectural constraint is that the `.md` file is canonical. Any tool can edit it — vim, sed, VS Code without the plugin, a script. When this happens, the daemon must ingest the change into the CRDT.

**Automerge ships this as a single API call:**

```rust
let mut tx = doc.transaction();
tx.update_text(&text_obj, &new_file_content)?;
tx.commit();
```

Internally (`rust/automerge/src/text_diff.rs`), this:
1. Runs a Myers diff between the CRDT's current text and the new file content
2. Aligns edit boundaries to grapheme clusters via `unicode_segmentation`
3. Emits Automerge splice operations that advance the document's `TextEncoding` cursor
4. Handles all edge cases: empty documents, complete rewrites, multi-byte characters, BOM handling

**No other CRDT library provides this.** With yrs, Diamond Types, or Loro, the developer must:
1. Compute the diff externally (using `similar`, `diff-match-patch`, or similar)
2. Translate each diff hunk into the library's operation format
3. Handle offset adjustments as earlier hunks shift positions
4. Handle encoding mismatches (library expects UTF-16 offsets; file is UTF-8)
5. Handle grapheme boundaries (splitting a multi-byte character produces invalid state)
6. Test extensively for edge cases the library's authors already solved

This is ~200-400 lines of careful, bug-prone code on buzzy's most critical path — the path that fires every time any user edits a file outside the plugin. Automerge eliminates this entirely.

---

## Secondary Factors

### Peritext marks (comment anchoring)

Post-MVP, buzzy needs inline comments anchored to text ranges that survive concurrent edits. Automerge ships Peritext-derived marks:

- Marks attach to character opIds with `before`/`after` qualifiers
- Bold spans grow under concurrent insertion at boundaries (`before` qualifier)
- Comments and links do NOT grow (`after` qualifier) — precisely correct for annotations
- Tombstones survive character deletion — an anchor outlives its text
- Overlapping comments coexist (additive, not last-write-wins)

With yrs, comment anchoring requires `RelativePosition` — a position expressed as an item ID + offset. This survives edits but:
- Has no before/after qualifier semantics (all anchors behave the same at boundaries)
- Requires manual management of overlapping annotations
- Is not a first-class marks system — it's a positioning primitive

With Loro, there's a separate marks system but it's not Peritext-derived and the semantics differ. With Diamond Types and raw Fugue, there's nothing — you'd build the entire annotation layer from scratch.

### Full operation history

Automerge retains every change by default. The entire edit history is available:
- Who wrote what, when (per-change actor ID + timestamp)
- Time-travel to any point in document history
- Selective revert of specific changes without affecting later edits
- Operation-stream analysis for AI features (edit velocity, collaboration patterns, staleness detection)

yrs explicitly trades history for performance — once an update is applied, the individual operations that composed it are not recoverable. This is correct for applications that don't need history, but buzzy's AI features (protocol research §AI Integration) depend on it.

### Published binary format with forward compatibility

The Automerge binary format spec (`automerge.org/automerge-binary-format-spec/`) guarantees:

> Unknown columns MUST be retained through read-write cycles. Unknown value type tags and unknown action codes are similarly preserved.

This means a `.buzzy/state/<uuid>.bin` file written by daemon v1.0 will survive read-write by daemon v2.0 without lossy conversion, even if v2.0 adds new operation types (block markers, comment anchors, AI attribution fields). No migration code needed; no version-tagged envelope needed.

yrs has no formal spec — the format is defined by the implementation. Forward compatibility is not guaranteed by specification; it's an emergent property of the implementation's backwards-compat discipline.

### Sync protocol

Automerge's Bloom-filter delta exchange:
- Each peer sends a message containing their frontier hashes + a Bloom filter of known change hashes
- The other peer identifies which changes might be missing and sends them
- Repeat until convergence (typically 1-2 round trips)
- Compaction shortcut: if send-set > ~⅓ of the graph, substitute a full `doc.save()` snapshot

This is tightly integrated with the CRDT format — the sync state is maintained per-peer and persists across sessions (`shared_heads`). Reconnecting after days offline is a single sync exchange, not a full retransmission.

Using Automerge's CRDT with a different sync protocol would require reimplementing the state tracking and Bloom filter logic — effectively forking the sync layer while depending on the CRDT layer.

---

## What We Give Up

### 1. Maximal non-interleaving (Fugue's property)

When two users concurrently type character runs at the same position, Automerge's RGA may order their runs in a way that's correct but not optimal — e.g., "Alice's runBob's run" vs "Bob's runAlice's run" depending on opIds. Fugue guarantees runs never interleave character-by-character.

**Why this is acceptable:** In practice, real-time collaboration shows users each other's cursors — they naturally avoid typing at the same position. In offline divergence, same-position conflicts are rare (users typically edit different sections). The interleaving anomaly is a theoretical concern that rarely manifests as a user-visible problem.

**Mitigation:** If dogfooding reveals interleaving as a real issue, the daemon architecture is CRDT-library-agnostic. Swapping Automerge for Loro (Fugue-based) is expensive but not architectural — the socket protocol, file watcher, and networking are unchanged.

### 2. Steady-state memory (Eg-walker's advantage)

Automerge's memory is O(operations) — proportional to edit history, not document size. A 10KB document with 100K edits consumes significantly more memory than the same document with 100 edits. Eg-walker (Diamond Types) achieves O(document) steady-state by storing only the event graph and computing merge on-demand.

**Why this is acceptable:** Automerge 3.x compaction (`doc.save()` produces a snapshot that front-loads a materialised state) largely mitigates cold-load time. For a 10KB meeting note with a year of edits, the sidecar might be 200-500KB — acceptable for desktop/laptop storage. If long-lived documents prove problematic, Automerge's garbage collection (compacting old history while preserving the snapshot) addresses it without changing the engine.

**Monitoring:** Benchmark sidecar sizes at Month 3 of dogfooding. If average sidecar exceeds 10x document size, evaluate compaction settings or Loro migration.

### 3. Yjs editor bindings ecosystem

Yjs has pre-built bindings for ProseMirror, Tiptap, Quill, CodeMirror 6, Monaco, Slate, Remirror, and Milkdown. Automerge has ProseMirror and CodeMirror 6 bindings but fewer options.

**Why this is acceptable:** buzzy is a daemon. The Obsidian plugin communicates via JSON-RPC over a Unix socket — it never imports the CRDT library. The plugin is a thin CodeMirror 6 extension (~200 lines) that sends position-based operations and receives position-based updates. The CRDT library choice is invisible to plugin authors.

### 4. Per-user undo (Yjs's UndoManager)

Yjs ships `Y.UndoManager` with transaction-origin filtering — only operations tagged with a local origin land on the undo stack; remote operations are excluded. This is the most practical multi-user undo in the field.

**Why this is acceptable:** Per-user undo is post-MVP. Automerge's full history retention makes selective undo *possible* (compute an inverse patch for a specific change), even if the ergonomics aren't as polished as Yjs's first-class API. Building a per-user undo layer on top of Automerge's history is estimated at 2-3 weeks of work when the feature is needed.

### 5. Raw performance

Yjs benchmarks faster than Automerge on most operations due to RLE-compressed Items. Diamond Types benchmarks faster than both due to the event-graph architecture.

**Why this is acceptable:** buzzy's performance target is sub-500ms edit propagation on LAN. Automerge 3.x achieves single-digit-millisecond local operations for typical document sizes (sub-100KB). The daemon is not the bottleneck — network latency, file I/O, and editor rendering dominate the critical path. If Automerge proves slow on specific workloads (100KB+ documents with deep history), profiling will identify whether the CRDT engine or another component is the bottleneck.

---

## Conditions for Revisiting This Decision

| Trigger | Alternative to evaluate | Timeline |
|---------|------------------------|----------|
| Sidecar sizes exceed 10x document size after 6 months of edits | Automerge compaction tuning; if insufficient, Loro (Eg-walker-adjacent memory model) | Month 6 |
| Interleaving reported as user-visible problem in dogfooding | Loro (Fugue-based, provably non-interleaving) | Month 4+ |
| Automerge `update_text` produces incorrect ops on specific markdown patterns | File bug upstream; if unresolved in 2 weeks, evaluate custom diff layer | Anytime |
| Automerge maintenance stalls (no releases for 6+ months) | yrs (most mature alternative); community fork | Ongoing |
| Mobile/embedded deployment requires O(document) memory | Diamond Types / Eg-walker (if matured by then) | Post-MVP |
| Performance profiling shows Automerge is the bottleneck at >50ms per operation | yrs (fastest); Loro; or Automerge with custom optimisation | Month 3 |

---

## Summary

Automerge-rs wins because it's the only option that covers all six requirements without custom code:

1. **Rust-native** — the reference implementation IS Rust
2. **External edit ingestion** — `Transaction::update_text` (one API call)
3. **Comment anchoring** — Peritext marks (built-in, correct semantics)
4. **Operation history** — full retention by default
5. **Published format** — forward-compatible binary spec
6. **Sync protocol** — Bloom-filter delta exchange (built-in)

The trade-offs (no maximal non-interleaving, higher memory than Eg-walker, fewer editor bindings than Yjs, no first-class undo manager) are either acceptable for the product's use case, solvable within the architecture, or deferrable to post-MVP evaluation.

The decision is not permanent. The daemon's layered architecture (socket protocol → document manager → CRDT engine) means the CRDT engine can be swapped without changing the editor plugins, networking, or file watcher. But the default is Automerge unless a specific trigger fires.
