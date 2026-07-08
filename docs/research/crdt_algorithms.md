# Text CRDT Algorithms — State of the Art (2025-2026)

A technical comparison of the major CRDT algorithms used for collaborative text editing, prepared as reference material for the buzzy design document.

Compiled from primary sources: [automerge.org](https://automerge.org), [docs.yjs.dev](https://docs.yjs.dev), the Fugue paper (arXiv 2305.00583, published *IEEE Transactions on Parallel and Distributed Systems* vol. 36 no. 11, Nov 2025), the Eg-walker paper (arXiv 2409.14252, EuroSys 2025, DOI 10.1145/3689031.3696076), [github.com/josephg/diamond-types](https://github.com/josephg/diamond-types), [github.com/automerge/automerge](https://github.com/automerge/automerge), and [inkandswitch.com/peritext](https://www.inkandswitch.com/peritext/).

**Coverage caveat:** the arXiv abstract pages and the Yjs docs landing page did not expose all deep-internal mechanics. Where a claim depends on the full paper or a secondary source (blog posts, `INTERNALS.md`), it is noted rather than fabricated. Numeric performance claims are quoted from the source and not independently benchmarked here.

---

## 1. RGA (Replicated Growable Array) — Automerge

**Core approach.** Each character is an immutable node in a causal tree; every insert names a parent character (its left neighbour at insertion time) and carries a globally-unique opId `(counter, actorId)`. Concurrent inserts under the same parent are ordered deterministically by opId, tie-broken by actorId. Deletions leave tombstones.

**Concurrent-insert semantics.** When two peers each type a run of characters at the same cursor position, RGA groups each user's run under the same parent, and orders the two runs against each other by opId. Consequence: characters within one user's run stay contiguous, but the two runs sit side-by-side as two blocks; older algorithms could produce interleaving *within* a run in certain re-parenting scenarios — RGA's specific parent-choice rule mostly avoids this but does not satisfy the maximal non-interleaving property proven for Fugue.

**Performance.** Automerge 3 (current stable — v3.2.6, Apr 2026) reports "~10× reduction in memory usage" vs. Automerge 2, with a columnar binary store on disk and in memory. History is retained by default ("every change is remembered"). Op-log grows with edit count, not document size; loading a long-lived document historically meant replaying every op, though 3.x front-loads a materialised snapshot.

**Interleaving.** Grouped by author within a run; between concurrent runs, ordering is by opId — measurably worse than Fugue on adversarial cases per Weidner's paper.

**Structured text.** The core is a flat sequence, but Automerge exposes `Automerge.Text` with **rich-text marks** (bold/italic/link, spans over character ranges) integrating Peritext-style semantics; a separate **block-tree** representation for headings/paragraphs/lists (via block markers embedded in the sequence + `patches` API) ships with ProseMirror and CodeMirror plugins. The Ink & Switch team drove both.

**Maturity.** Rust core with FFI. Bindings: JS/WASM (`@automerge/automerge`), C, Swift, Python, Java (ports). Formally proved parts using Isabelle. 6.4k GitHub stars, maintained full-time by Alex Good and Orion Henry with Ink & Switch collaborators.

**Wire format / sync.** Documented columnar binary format (published spec at automerge.org/automerge-binary-format-spec). Sync protocol is a Bloom-filter-based delta exchange used by `automerge-repo`; suitable for P2P, client-server, files, or "email attachments."

---

## 2. YATA (Yet Another Transformation Approach) — Yjs

**Core approach.** Each item ("Item" in Yjs internals) is a linked-list node holding `origin` (left neighbour at insertion time) and `rightOrigin` (right neighbour). Concurrent inserts sharing the same origin/rightOrigin are ordered by client id, tie-broken by clock. Deletes are separate delete-sets. Yjs represents runs of consecutive inserts by one client as a single Item that can be split on demand — the source of much of its speed and memory efficiency.

**Concurrent-insert semantics.** YATA's rightOrigin plus a specific conflict-resolution rule keeps concurrent runs from the same origin from interleaving character-by-character; they alternate at block boundaries by client id. Same *empirical* result as RGA in most cases; the Fugue paper argues both can still interleave in edge cases and neither is provably maximally non-interleaving.

**Performance.** Yjs positions itself as "the fastest CRDT implementation" (yjs/benchmarks). In practice: constant-time local ops via run-length encoding of Items, single-digit-MB memory for large docs, and small binary updates. Not history-oriented — it stores the CRDT graph, not an op log, so undo/redo depends on the UndoManager rather than time-travel.

**Interleaving.** Same class as RGA: not provably non-interleaving but well-behaved for typical human editing.

**Structured text.** Rich set of shared types: `Y.Text` (with inline attributes for bold/italic/etc., Delta-compatible), `Y.Array`, `Y.Map`, `Y.XmlFragment`/`Y.XmlElement`/`Y.XmlText` for tree-structured documents. Battle-tested bindings for ProseMirror (`y-prosemirror`), Tiptap, Quill, CodeMirror, Monaco, Slate, Remirror, Milkdown.

**Undo/redo.** `Y.UndoManager` is per-user via **tracked transaction origins**: only ops tagged with a local origin land on the local undo stack; remote ops are ignored. Merges rapid edits by `captureTimeout` (default 500 ms); stack-item events allow attaching cursor/selection metadata.

**Maturity.** JS/TypeScript reference implementation; Rust port `y-crdt` (yrs) with bindings for Python (y-py), Ruby (y-rb), .NET (Ycs), Swift, and iOS. Mature ecosystem (`y-websocket`, `y-webrtc`, `y-indexeddb`, `y-redis`, `hocuspocus`).

**Wire format / sync.** Well-defined binary format; **Y.Protocols** covers sync (state vectors + updates) and **awareness** (ephemeral presence — cursors, selections, user info — separate from the CRDT).

---

## 3. Fugue — Weidner & Kleppmann (2023, journal 2025)

**Core contribution.** Formalises a defect present in RGA and YATA under the name *interleaving anomaly*: when two users concurrently insert text passages at the same position, existing algorithms can merge them into a jumbled interleaving. Fugue introduces the correctness criterion **maximal non-interleaving** and proves FugueMax satisfies it.

**Algorithm shape.** A list-with-tree structure where each inserted item stores its origin position; two variants — Fugue (lightweight) and FugueMax (satisfies the strong property). Uses ordering hints that prevent runs from splitting across each other under concurrency.

**Performance.** The paper claims performance "comparable to state-of-the-art CRDT libraries for text editing" — the mechanism is O(log n)-ish per op via balanced tree traversal, no worse than RGA/YATA asymptotically.

**Interleaving.** The defining feature: provably non-interleaving concurrent runs. This is the algorithm's headline benefit.

**Structured text.** Not addressed in the paper — Fugue is a plain-text sequence CRDT. Any structured-text layer would sit on top (analogous to Peritext-over-RGA).

**Maturity.** Reference TypeScript implementation exists (`fugue` on npm, part of Weidner's Collabs library). Not the primary sequence CRDT in any large collaborative product yet, but the algorithm has been adopted as inspiration inside Loro and referenced in more recent CRDT designs.

**Wire format / sync.** No standardised wire format — implementation-defined. Collabs provides one; Loro provides another. Not a plug-compatible replacement for Yjs or Automerge.

---

## 4. Diamond Types — Joseph Gentle

**Algorithm.** Rust implementation of what its README calls "the world's fastest CRDT," internally described in `INTERNALS.md`. Two operation modes:

1. A traditional CRDT list (a positional-CRDT variant, similar in spirit to YATA/Fugue).
2. An **event-graph** encoding — this is where **eg-walker** originated. Diamond Types was the first shipping implementation of the eg-walker idea (see §6).

**Performance.** Gentle's blog reports 5000× speedup over reference CRDTs in early benchmarks, with a further 10–80× improvement since. Uses run-length encoded ops keyed by `(clientId, seq)` and a B-tree over live characters for O(log n) inserts.

**Maturity.** WIP per repo; 1.8k stars, 1.3k commits, 3 releases. Note in the README: "the package published to cargo is quite out of date, both in terms of API and performance." Production use exists but is limited — mostly experimental deployments.

**Wire format / sync.** `BINARY.md` describes a compact binary log of edits. Not a widely-implemented standard; interop is essentially "Diamond Types ↔ Diamond Types."

**Language availability.** Rust primary; `diamond-types-node` and `diamond-types-web` on npm via WASM; Swift build script exists. No Python/Java/Go bindings.

**Structured text.** Plain text only in the current release; the `more_types` branch is a WIP for JSON-style data.

---

## 5. Peritext — Ink & Switch

**Scope.** Not a sequence CRDT itself — it's a **rich-text formatting layer** that sits on top of an RGA-like plain-text CRDT. Solves the specific problem of concurrent inline formatting.

**Core mechanism.** Formatting is expressed as `addMark(start, end, markType)` / `removeMark` operations rather than inline tags. Endpoints are anchored to character opIds with a `before`/`after` sentinel — a bold span uses `before` on both ends (so trailing typed text joins the bold); a link uses `after` on the end (so trailing text stays outside the link). Each character stores `markOpsBefore` and `markOpsAfter` sets; application is commutative.

**Conflict resolution.**

- Same mark on overlapping ranges (bold + bold) → union.
- Compatible marks (bold + italic) → both apply on overlap.
- Conflicting marks (two highlight colours) → last-write-wins by Lamport opId.
- **Comments** are additive rather than LWW: overlapping comments coexist — precisely what makes it a fit for anchored annotations.

**Structured text.** In-scope: inline formatting (bold, italic, link, colour, comments). **Out of scope**: block-level structure — headings, lists, tables, nested blocks — explicitly deferred to future work in the paper.

**Maturity.** TypeScript prototype at github.com/inkandswitch/peritext with a ProseMirror UI and randomised convergence tests. **The algorithm has been integrated into Automerge** as the basis for `Automerge.Text` marks and the rich-text/blocks work — Peritext concepts are the reason Automerge ships production rich-text.

**Wire format / sync.** Uses the host CRDT's wire format (Automerge's binary format, in the integrated case).

---

## 6. Eg-walker (Event Graph Walker) — Kleppmann et al.

**Positioning.** Published at EuroSys 2025 (Kleppmann, Gentle, Feltman). Explicitly attempts to be *neither* OT nor a traditional CRDT — it stores an **event graph** (the raw log of edits + causal links) and *transforms events on read/merge* rather than maintaining permanent CRDT tombstones and metadata.

**Key idea.** Traditional CRDTs pay a permanent metadata cost — every character carries an opId, and deletions leave tombstones forever. Eg-walker stores just the event log; at merge time it walks the event graph and produces the correct merged result, but the *steady-state* document is a plain string (or nearly so). This is fundamentally the trick Diamond Types uses.

**Performance claims (per the paper's abstract).**

- Steady-state memory: **order of magnitude less** than existing CRDTs.
- Load-from-disk: **orders of magnitude faster** than CRDTs.
- Merging long-running branches: **orders of magnitude faster** than OT.
- Worst case: comparable to existing CRDT algorithms.

**Interleaving.** The paper does not headline a maximal-non-interleaving proof; the merge semantics track the underlying CRDT rules (typically YATA-family). This is a memory/perf win, not an interleaving-quality win.

**Structured text.** The published algorithm is for plain text sequences. Diamond Types is exploring JSON-style extensions on top.

**Maturity.** Diamond Types is the reference implementation; a JavaScript port exists (`egwalker` prototypes). Not yet in Automerge or Yjs. Actively-published research (2024-2025) with a 2025 EuroSys paper.

**Wire format / sync.** Diamond Types' `BINARY.md` describes one concrete format. No cross-implementation standard.

---

## Extension Areas

### Block-level / tree-structured documents

| Algorithm | State |
|---|---|
| **Automerge** | Ships. Block markers embedded in the text CRDT plus a `Map`/`List` layer; ProseMirror & CodeMirror bindings; "Blocks" model formalised in 2024. |
| **Yjs** | Ships. `Y.XmlFragment`/`Y.XmlElement` express arbitrary trees; `y-prosemirror` maps ProseMirror docs onto them 1:1. Most mature option. |
| **Peritext** | Explicitly out of scope; only inline formatting in a single paragraph. |
| **Fugue** | Not addressed. |
| **Diamond Types** | Not yet — plain text release only; `more_types` branch WIP. |
| **Eg-walker** | Not addressed in the paper. |

The general pattern for block trees is: use a sequence CRDT for text within a block, plus a separate CRDT (Map/List) for the block structure itself. Move operations across blocks remain an open problem — see Kleppmann's "Moving Elements in List CRDTs" (2020) — no algorithm covered here handles concurrent block moves without conflict.

### Anchored annotations (comments on text ranges)

| Algorithm | State |
|---|---|
| **Peritext** | First-class. Comments are additive marks with stable opId anchors; survive edits, deletion of anchor character, and concurrent overlapping comments. |
| **Automerge** | Inherits Peritext semantics via marks; anchors survive edits (endpoint moves with adjacent character; if character is deleted, anchor persists on tombstone). |
| **Yjs** | Anchors via **RelativePosition** — a position expressed as an item ID + offset that survives concurrent edits. Comments/annotations are typically layered on top by users (e.g., `y-prosemirror` community plugins) rather than provided as a first-class type. |
| **Fugue / Diamond Types / Eg-walker** | Not addressed — would need a Peritext-like layer built on top. |

### Undo/redo in multi-user contexts

| Algorithm | State |
|---|---|
| **Yjs** | `Y.UndoManager` filters by transaction origin; per-user local stacks; only tracked origins land on the stack, so remote edits are never undone. Merges rapid edits by `captureTimeout`. |
| **Automerge** | Undo/redo via change replay: because every change is retained, you can produce an inverse patch. Automerge has shipped undo primitives; per-user isolation is caller-managed. |
| **Peritext** | Reversibility is a design goal (marks are add/remove ops); no dedicated user-scoped stack. |
| **Fugue / Diamond Types / Eg-walker** | Not addressed at the algorithm layer. Diamond Types' event graph makes selective undo tractable in principle. |

Correct multi-user undo — "undo only *my* edits, without undoing subsequent edits on top of mine" — remains the harder unsolved problem across the board; Yjs's origin-filtering is the most practical current solution but has known edge cases when the local user's op depends causally on remote ops.

---

## Summary matrix

| Property | RGA/Automerge | YATA/Yjs | Fugue | Diamond Types | Peritext | Eg-walker |
|---|---|---|---|---|---|---|
| Concurrent-insert ordering | opId under parent | client id + rightOrigin | tree-structured, provably non-interleaving | eg-walker (YATA-family semantics) | inherits underlying CRDT | eg-walker (YATA-family semantics) |
| Maximally non-interleaving | No | No | **Yes (FugueMax)** | No | N/A | No |
| Memory scaling | O(ops) history; 3.x much reduced | O(items), RLE-compressed | O(items) | **O(document) steady state** | Delegated to host CRDT | **O(document) steady state** |
| Load time | Linear in history (3.x snapshot mitigates) | Linear in items | Linear | **~document size** | N/A | **~document size** |
| Wire format | Columnar binary, spec'd | Binary, spec'd (Y.Protocols) | Impl-defined | Impl-defined (BINARY.md) | Host's format | Impl-defined |
| Sync protocol | `automerge-repo` (Bloom-filter delta) | y-protocols sync + awareness | None standard | None standard | Host's | None standard |
| Rich-text marks | **Yes (Peritext-derived)** | **Yes (Y.Text attributes)** | No | No | **Yes (the point)** | No |
| Block trees | Yes (block markers + Map layer) | **Yes (Y.Xml\*)** | No | No | No (explicit non-goal) | No |
| Anchored comments | Yes (Peritext marks) | Via RelativePosition (DIY) | No | No | **Yes (first-class)** | No |
| Per-user undo | Yes (call-managed) | **Yes (UndoManager)** | No | No | No | No |
| Languages | Rust core + JS/C/Swift/Python/Java | JS/TS + Rust port + many bindings | TS reference | Rust + WASM (JS) | TS prototype | Rust (Diamond Types) |
| Production readiness | **High** (v3.2.6) | **High** | Research / small deployments | WIP, single-maintainer | Prototype (integrated into Automerge) | Research / Diamond Types |

---

## Recommendation for a design document

For a structured-text collaborative product being built today:

- **Default choice: Yjs.** Most mature ecosystem, best editor bindings (ProseMirror/Tiptap), tree-structured types (`Y.XmlFragment`), per-user undo, awareness protocol, cross-language bindings, and lowest wire overhead in practice. Interleaving is imperfect but rarely user-visible.

- **Choose Automerge if** you need first-class history/time-travel, Peritext-style rich-text marks with strong semantics, or a Rust-first stack. Automerge 3.x closed most of the historical memory/perf gap vs. Yjs; the sync protocol is well-suited to P2P.

- **Watch Eg-walker / Diamond Types** if steady-state memory and cold-load time matter more than history features. The algorithmic story is compelling but the ecosystem is thin — a bet on 2026-2027, not 2025.

- **Fugue** matters if the product is heavily concurrent editing of prose (e.g. two writers typing in the same paragraph simultaneously) and interleaving quality is a differentiator. Otherwise its practical delta over YATA is small.

- **Peritext is not an "algorithm choice"** — it's a layer. If rich text with anchored comments is required, either use Automerge (which ships Peritext internally) or plan to build a Peritext-style layer on your chosen sequence CRDT.

Unresolved across the field: concurrent block moves (Kleppmann 2020 has partial answers, none shipping), true per-user selective undo, and interop between different CRDT wire formats.
