---
title: "buzzy — Document Storage Formats and Sidecar Patterns"
subtitle: "Research consolidation for the buzzy design doc"
date: 2026-07-06
status: research
---

# Buzzy — Document Storage Formats and Sidecar Patterns

## Intro

This document consolidates research on how existing local-first collaborative tools handle on-disk document representation, and what that implies for Buzzy — a daemon that provides real-time CRDT-based collaboration over plain markdown files, using a sidecar file next to each `.md` to store CRDT state. The research covers six areas: Ink & Switch's local-first prior art, AFFiNE / BlockSuite, Outline, Obsidian and Logseq vault conventions, markdown standards and sidecar file patterns, and the on-disk formats of the two dominant CRDT libraries (Automerge and Yjs). All findings are grounded in primary sources — spec pages, repository READMEs, and codebase reads on `main` — with URLs cited where relevant.

The document ends with answers to four buzzy-specific questions posed by the design lead:

1. What format should the sidecar file use? (Automerge binary, custom, inspectable?)
2. How do you maintain the invariant that the `.md` file is always valid and readable without the sidecar?
3. How do other tools handle the "file modified externally" problem?
4. Are there any standards or conventions for "this plain text file has CRDT metadata attached"?

A consolidated "alternatives considered" table follows for direct paste into the buzzy design doc.

---

## 1. Ink & Switch prior art

### 1.1 "Local-first software" (2019, Kleppmann et al.)

URL: `https://www.inkandswitch.com/essay/local-first/`

The paper enumerates seven ideals — "no spinners", "not trapped on one device", "network is optional", "seamless collaboration", "the Long Now", "security & privacy by default", "you retain ultimate ownership". Directly relevant quote:

> "Since local-first applications store the primary copy of their data in each device's local filesystem, the user can read and write this data anytime, even while offline."

On file formats it only notes that Library of Congress "recommends XML, JSON, or SQLite" for archival, and that "some file formats (such as plain text, JPEG, and PDF) are so ubiquitous that they will probably be readable for centuries to come".

**What the paper does not address:**

- File-modified-externally reconciliation
- Sidecar files
- Hybrid plaintext+CRDT storage
- Whether the CRDT-backed file is human-readable without the runtime

These are open questions the paper leaves for future work. buzzy is a candidate answer.

### 1.2 Peritext (2021)

URL: `https://www.inkandswitch.com/peritext/`

Peritext is a CRDT for rich-text collaboration. It defines the anchor mechanism buzzy needs for comment stability.

**Sticky positions.** Marks anchor to **character opIds** — Lamport `(counter, nodeId)` pairs — not offsets, with a **before/after** qualifier on each endpoint:

- Bold ends `before` the next character → grows when text is inserted at the boundary.
- Links and comments end `after` the last character → do not grow.

**Tombstones survive deletion.** An anchor outlives its underlying character:

> "A character that has been marked as deleted is called a tombstone."
> "When a character is deleted, we preserve any attached operations."

Overlapping comments on the same character coexist — not resolved via last-writer-wins.

**Critical: Peritext explicitly rejects "markdown in a plain-text CRDT".** A section titled "Markdown in a plain text CRDT" walks through concurrent-bold cases producing broken output like `**The **fox** jumped.**`. HTML control chars and JSON trees are similarly rejected. Their conclusion: keep text and formatting *separate* — a plain-text CRDT sequence plus mark operations referencing character opIds (inspired by atjson). This is exactly what Peritext does.

**Implication for buzzy:** the buzzy design doc's current "Challenge #1: CRDT over structured markdown" bullet correctly names this risk. Peritext is the *evidence* that simpler approaches don't work. Cite it in the design doc.

### 1.3 Cambria (2020) — schema evolution for CRDTs

URL: `https://github.com/inkandswitch/cambria`

Bidirectional translation of JSON between related schemas via YAML/JSON lenses. Relevant if buzzy ever needs to migrate block schema across peers on different versions. Lenses require JSON, not opaque binary — a possible future path if buzzy projects its sidecar to JSON for schema migration. Marked as research-prototype quality ("still immature software, and isn't yet ready for production use").

### 1.4 Pushpin (2020)

URL: `https://github.com/inkandswitch/pushpin`

Archived. Storage was Automerge/hypermerge binary log format in a platform-dependent shared location (`~/Library/Application Support/pushpin/`). No plain-text canonical form, no sidecar pattern, no file-modified-externally story. Historical prior art only.

### 1.5 Upwelling (2023) — closest philosophical prior art

URL: `https://www.inkandswitch.com/upwelling/`

Combines real-time collaboration with version control for prose writers ("creative privacy"). Explicitly went CRDT-only.

- **Storage:** single tarball bundling a stack of drafts plus a metadata Automerge doc.
- **Rich-text:** Peritext.
- **UI:** ProseMirror.
- **Sync:** Automerge's built-in protocol.
- **Merge philosophy:** rejects Git-style diff/merge for prose in favour of CRDT auto-merge.

The authors self-critique that they wrote the Upwelling essay itself "in multiple Google Docs (in Markdown, no less) and has dozens of commits in a Git repository" — an admission the tool doesn't yet solve the interop-with-plaintext problem.

**Future-work quote worth citing in buzzy's positioning:** they want *"a file format or exchange protocol that makes it possible for writers to use the writing software of their choice."* **buzzy is a candidate answer.**

### 1.6 Automerge binary format spec

URL: `https://automerge.org/automerge-binary-format-spec/`

- **Structure:** length-delimited chunks. Three chunk types — Document `0x00`, Change `0x01`, Compressed change `0x02`.
- **Magic bytes:** `[0x85, 0x6f, 0x4a, 0x83]` per chunk, followed by a 4-byte SHA-256-prefix checksum, type byte, uLEB length.
- **Storage semantics:** snapshot-plus-full-history. "A document always contains a complete history of changes." Not pure append-only.
- **Compression:** columnar RLE, per-column optional DEFLATE, whole-chunk DEFLATE, delta encoding for monotonic sequences, empty columns omitted.
- **Forward compatibility:** the spec says unknown columns "MUST" be retained through read-write cycles. Unknown value type tags and unknown action codes are similarly preserved. **Load-bearing for buzzy:** a `.md.crdt` written by daemon vN survives read-write by daemon vN+1 without lossy conversion, even if the newer version adds fields.
- **Inspectability:** not casually — requires a parser that understands columnar RLE + LEB + DEFLATE. Authors recommend using their reference implementation but publish the spec so third-party parsers are possible.
- **GC:** not discussed. Format retains complete history by design.

---

## 2. AFFiNE and BlockSuite

Sources: `blocksuite.io/guide/data-synchronization.html`, `github.com/toeverything/AFFiNE`, `github.com/toeverything/blocksuite`, `github.com/toeverything/OctoBase`, and two AFFiNE dev-blog posts.

### 2.1 Stack

Three layered projects under `github.com/toeverything`:

| Layer | Role | Repo |
|---|---|---|
| AFFiNE | Product shell (Electron desktop, browser, mobile) | `toeverything/AFFiNE` |
| BlockSuite | Block-level editor + CRDT-backed document model | `toeverything/blocksuite` |
| OctoBase | Rust storage/sync engine, embeddable + standalone | `toeverything/OctoBase` |

CRDT is **Yjs** (not Automerge). OctoBase implements a Rust-native Yjs port called **`y-octo`** with a companion codec crate `jwst-codec`. License: MPL-2.0. Latest BlockSuite release noted: v0.22.4 (Jul 2025).

### 2.2 Storage

**Storage is opaque binary CRDT data — not markdown, not JSON.** Concrete backends:

- **Browser client:** `IndexedDBProvider` (via `y-indexeddb`).
- **Desktop/native:** `SQLiteProvider`.
- **Self-hosted server:** Postgres for CRDT storage, Redis for sync infrastructure, plus a blob store (filesystem, S3, or R2).
- **OctoBase (embedded):** pluggable adapters — SQLite and Postgres confirmed; S3 in progress.

Self-hosted volumes are configured via three env vars: `DB_DATA_LOCATION`, `UPLOAD_LOCATION`, `CONFIG_LOCATION`.

No user-facing "one `.md` per note" layout exists. A workspace is a Yjs document persisted as binary CRDT updates. Users can "choose where to store your workspace as a single file" that doubles as a backup — this is a workspace-level export, not a per-note file.

### 2.3 Block model

A document is a `Doc` managing an independent **block tree**. Each block has:

- A `flavour` string using `"namespace:name"` (preset editable blocks use `affine:*`).
- A schema (`BlockSchema`) declaring typed props and allowed nesting.
- A `children` array (the tree).
- An `id`.

Example:

```ts
const rootId = doc.addBlock('affine:page');
const noteId = doc.addBlock('affine:note', {}, rootId);
const paragraphId = doc.addBlock('affine:paragraph', {}, noteId, 0);
doc.updateBlock(model, { type: 'h1' });  // heading is a paragraph subtype
```

Headings are a `type` variant on `affine:paragraph`, not a separate flavour. Each block owns its own flat inline text (a per-block `YText`) — nesting is expressed via the block tree, not via nested rich text. Selection paths are arrays of block IDs from root.

Key packages (all separately usable):

- `@blocksuite/store` — data/document layer, Yjs-based
- `@blocksuite/block-std` — framework-agnostic block modeling
- `@blocksuite/inline` — inline rich-text components
- `@blocksuite/blocks` — default block implementations
- `@blocksuite/presets` — plug-and-play editors (`PageEditor`, `EdgelessEditor`)

### 2.4 Sync protocol

Provider-based, transport-pluggable. Providers named in the docs:

- `IndexedDBProvider` — browser persistence
- `SQLiteProvider` — native persistence
- `WebSocketProvider` — server-relayed sync
- `WebRTCProvider` — peer-to-peer

OctoBase's `libs/jwst-rpc/src/connector/` implements the transports on the Rust side: `tungstenite_socket.rs` (WebSocket, shipped), `webrtc.rs` (shipped), libp2p (in progress). Blob sync is REST.

Local edits are applied to the YDoc synchronously, regardless of network state. When any provider reconnects, updates fan out through the others; Yjs merges divergent histories deterministically. There is no manual conflict resolution.

Wire format: `y-protocols` binary incremental update messages, described by AFFiNE's own docs as "binary incremental update data" and compared to protobuf.

### 2.5 Snapshot vs live sync — architectural pattern worth borrowing

BlockSuite distinguishes two mechanisms cleanly:

| Snapshot API | Document Streaming |
|---|---|
| JSON representation of the block tree | Binary Yjs CRDT payloads |
| `job.docToSnapshot(doc)` / `snapshotToDoc(json)` | Providers attached to `doc.spaceDoc` |
| Point-in-time; feeds Adapter conversions (markdown, HTML) | Continuous, incremental sync |
| `ui = f(data)(state)` model | `ui = f(data)` model |

Markdown/HTML pass through an **Adapter/Transformer** layer sitting on top of snapshots. Round-trip fidelity is limited to what BlockSuite's block schema can express; edgeless canvas and embed blocks have no clean markdown equivalent.

### 2.6 Portability and external-edit handling

- **Portability today:** workspaces can be exported as a single file that other AFFiNE clients can open. Individual pages can be exported to markdown/HTML via the Adapter layer. The stored binary is not designed to be read outside AFFiNE.
- **External edits:** not addressed in any doc or blog post. AFFiNE assumes its own client is the sole writer to the CRDT store. There is no exposed plain-text file for editors like Obsidian/VS Code to touch.
- **Offline-to-online merge:** implicit through Yjs.

### 2.7 Two patterns worth borrowing for buzzy

1. **Provider abstraction.** A single YDoc attaches to multiple providers; updates fan out. buzzy's daemon can wear the same shape: a filesystem-backed local provider plus network providers.
2. **Snapshot ↔ live-sync separation.** If buzzy *inverts* the split — markdown *is* the snapshot; CRDT is the wire format / sidecar — the same architectural split maps cleanly.

**Docs gap noted:** AFFiNE and BlockSuite docs are surprisingly sparse on concrete schemas, table layouts, and the exact block-to-Yjs mapping. Most useful architectural detail comes from two blog posts rather than reference docs.

---

## 3. Outline

Sources extracted directly from `server/models/Document.ts`, `server/collaboration/PersistenceExtension.ts`, `server/collaboration/APIUpdateExtension.ts`, `server/commands/documentCollaborativeUpdater.ts`, `server/models/helpers/DocumentHelper.tsx`, `server/commands/documentImporter.ts` on `main` in `outline/outline`.

### 3.1 Storage model — structured-canonical, not markdown-canonical

Server-first. There is no on-disk file format; documents are Postgres rows managed by Sequelize. The `documents` table has three body columns:

| Column | Type | Role |
|---|---|---|
| `content` | `JSONB` (`ProsemirrorData`) | **Canonical.** Snapshot of the ProseMirror JSON at last save. |
| `state` | `BLOB` (`Uint8Array`) | Yjs binary CRDT state (`Y.encodeStateAsUpdate` output). Fallback when `content` is null; source of truth during live sessions. |
| `text` | `TEXT` | **Deprecated** legacy Markdown. No longer written. |

The model comment on `text` is unambiguous: `"@deprecated Use content instead, or DocumentHelper.toMarkdown if exporting lossy markdown."`

### 3.2 Schema

Assembled from `shared/editor/nodes/` and `shared/editor/marks/`. Dependencies pin the classic ProseMirror stack: `prosemirror-model 1.25`, `prosemirror-state`, `prosemirror-transform`, `prosemirror-tables`, `prosemirror-markdown 1.13`, `markdown-it 14`. No Tiptap, Remirror, Slate, or Lexical. Block types include the usual set plus Outline-specific attributes (comment marks, highlight colours, table `colwidth`) that markdown cannot represent — the codebase explicitly flags these as "non-markdown-representable" in `DocumentHelper.tsx`.

### 3.3 Real-time collaboration

Outline uses **Hocuspocus + Yjs + y-prosemirror**, not custom OT.

- `@hocuspocus/server 1.1.3`
- `@hocuspocus/provider`
- `@hocuspocus/extension-redis`
- `@hocuspocus/extension-throttle`
- `yjs 13.6`
- `y-prosemirror 1.3`
- `y-protocols`
- `y-indexeddb 9.0` (client-side, in-flight edits only)

**`onLoadDocument`:**

```ts
if (state) {
  const ydoc = new Y.Doc();
  Y.applyUpdate(ydoc, documentWithoutLock.state);
  return ydoc;
} else {
  ydoc = ProsemirrorHelper.toYDoc(document.content, fieldName);
  // ... persist derived state back
}
```

**`onStoreDocument` → `documentCollaborativeUpdater`:**

```ts
const state = Y.encodeStateAsUpdate(ydoc);
const content = Node.fromJSON(schema, yDocToProsemirrorJSON(ydoc, "default")).toJSON();
await document.update({ content, state: Buffer.from(state), ... }, { hooks: false });
```

Both `content` and `state` are written together every flush; `text` (markdown) is never regenerated in the collab path.

### 3.4 External-edit reconciliation — the closest working pattern

`APIUpdateExtension.ts` bridges REST API document edits with live collab sessions via **Redis pub/sub**:

1. REST update writes to Postgres.
2. `RedisAdapter.defaultClient.publish(channel, message)` fires.
3. Collab server (subscribed via `psubscribe(...:*)`) receives it, re-reads the DB.
4. **State-vector diff:**
   ```ts
   const currentStateVector = Y.encodeStateVector(document);
   const update = Y.encodeStateAsUpdate(dbYdoc, currentStateVector);
   if (update.length > 0) {
     Y.applyUpdate(document, update);
   }
   ```
5. Change propagates to all connected clients via normal Yjs sync.

**Directly relevant to buzzy's file-modified-externally problem, with one asymmetry:** Outline's "external" edit still comes from an API that speaks ProseMirror JSON, so it can compute a Yjs delta cleanly. buzzy's external edit is *plain text*, which means we have to first diff the raw markdown against the CRDT's rendered text, then translate the diff into synthetic CRDT splice ops — a strictly harder problem. Outline doesn't solve that; buzzy can borrow the state-vector-diff-then-`applyUpdate` mechanism but the "generate ops from text diff" step is on us.

### 3.5 Markdown fidelity — explicitly lossy in both directions

- **Export (`DocumentHelper.toMarkdown`)** uses `prosemirror-markdown`'s serializer. JSDoc: `"This is a lossy conversion and should only be used for export."` Comment marks, highlight colours, table `colwidth`, and other ProseMirror-only attributes are dropped.
- **Import (`server/commands/documentImporter.ts`)** delegates to `DocumentConverter.convert(...)` (built on `markdown-it 14`).
- **`mergeAttrs`** helper exists specifically to preserve non-markdown-representable attrs across round-trip — "attrs that cannot be represented in markdown, such as comment marks or highlight colours… values (colwidth, highlight colours, etc.) possibly lost in the round-trip."

**Cautionary takeaway:** Outline's schema evolved past what markdown can round-trip. buzzy needs either (a) discipline to keep the CRDT block schema constrained to markdown-representable constructs, or (b) accept the same "lossy on export" reality — but "lossy on export" means users lose data every time they open the file in vim, which breaks the buzzy invariant.

### 3.6 Offline / external-tool interop

No offline editing story beyond `y-indexeddb` scratch storage on the client. No filesystem sync. Editing an Outline document in Obsidian requires **export markdown → edit → re-import as new/updated doc**, which loses non-markdown attributes each cycle. No two-way sync path exists.

---

## 4. Obsidian and Logseq

### 4.1 Obsidian vault layout

```
MyVault/
├── Note.md
├── Sub/Another.md
└── .obsidian/                  ← per-vault config
    ├── workspace.json          ← current layout, updates constantly
    ├── workspaces.json
    ├── snippets/               ← CSS
    ├── plugins/
    │   └── <plugin-id>/
    │       ├── main.js
    │       ├── manifest.json
    │       └── data.json       ← plugin's own persisted state
    └── themes/
```

The `plugins/<id>/data.json` convention is documented in the developer API (`loadData()`/`saveData()`). Docs recommend gitignoring `workspace.json`/`workspaces.json` because they churn on every focus change. Obsidian also keeps state outside the vault: an OS-specific config dir plus an IndexedDB cache holding the metadata index and Sync state.

### 4.2 The critical dotfile constraint

**Obsidian treats dot-prefixed files and folders as non-existent** in the app: they don't appear in the file explorer, aren't indexed, and links to them resolve as broken. The only sanctioned dot-directories are `.obsidian/`, `.trash/`, and any folder set via the "Override config folder" setting (which must start with a dot). Obsidian Sync ignores hidden files by default. **This is the single most important design signal for buzzy.** The `obsidian-show-dotfiles` plugin exists precisely to expose them for users who deliberately want to see them.

### 4.3 Frontmatter / properties

YAML frontmatter delimited by `---`, with seven typed properties: Text, List, Number, Checkbox, Date, Date & time, Tags. `tags`, `aliases`, `cssclasses` are reserved. **No formal namespacing convention exists for third-party metadata.** Plugins that store per-note state typically use unclaimed frontmatter keys or write to their own `data.json` keyed by file path.

### 4.4 Obsidian sync approaches

- **Obsidian Sync** (proprietary): selectively syncs "files and settings"; can conflict with Dropbox/Google Drive. Mechanism not publicly documented as CRDT; behaviour suggests snapshot-based (uncertain).
- **obsidian-livesync** (vrtmrz): CouchDB / S3 / R2 / WebRTC-P2P backend. Uses **PouchDB** locally and chunk-based streaming over CouchDB's `_changes` feed. **Not CRDT** — README describes "automatic merging of simple conflicts", consistent with CouchDB's revision-tree model.
- **Relay** (System 3): **explicitly Yjs CRDT**, server is a fork of y-sweet. CRDT state held server-side and in memory; on disk it stays a plain `.md`.
- **Peerdraft**: real-time cursors, E2E-encrypted P2P for ephemeral sessions. Behaviour is consistent with Yjs (unverified).

### 4.5 Logseq graph layout

```
MyGraph/
├── journals/           ← daily notes, YYYY_MM_DD.md (or .org)
│   └── 2020_05_14.org
├── pages/              ← named pages, one file per page
│   ├── Properties.md
│   └── Block Reference.md
├── assets/             ← binaries (images, PDFs)
├── whiteboards/        ← whiteboard JSON
└── logseq/             ← per-graph config
    ├── config.edn
    ├── custom.css
    └── metadata.edn
```

Journals use underscore-separated dates; pages use the page title as filename. Logseq supports both Markdown and Org-mode within one graph.

### 4.6 Logseq block-level model on top of plain markdown

Everything is a **bullet-list item** (`- ...`), which is valid CommonMark; block metadata is embedded inline using `key:: value` syntax, one property per line. Example:

```markdown
type:: [[Feature]]
description:: Annotates any block or page with multiple pairs of values

- ## Usage
	- Property naming rules:
	  collapsed:: true
		- ### Property values
		  id:: 6356e902-3b7b-4cb2-8c3e-6a904c813c40
```

Rules:

- **Page properties** live in the first block (no leading `-`), acting as frontmatter without `---` delimiters — terminated by first blank line.
- **Block properties** are indented under a bullet.
- **Block IDs** stored inline as `id:: <uuid-v4>`, created lazily when first referenced.
- **Block references** use `((uuid))`; embeds use `{{embed ((uuid))}}`.
- Property names case-insensitive; values can't contain newlines.

`key:: value` isn't standard CommonMark, so a non-Logseq renderer shows it as literal text — small readability cost, but files stay grep-able and diff-friendly.

### 4.7 Logseq sync approaches

- **Logseq Sync** (file-based version, proprietary): end-to-end encrypted file sync on AWS. Not CRDT (uncertain — no public architecture doc).
- **DB version + RTC**: newer SQLite-backed "DB graphs" (beta) ship with **Real-Time Collaboration (RTC) sync in alpha**. README explicitly warns "data loss is possible." **This model gives up plain-file storage entirely.**

No CRDT is documented for the file-based version. The DB version's schema suggests a tree-CRDT is plausible but is unverified.

### 4.8 Design lessons distilled

1. **Never rely on dotfile sidecars in an Obsidian vault.** `.notes.md.crdt` will be silently ignored by Obsidian's file explorer, Sync, search, and graph. Either (a) place all CRDT state under **`.obsidian/plugins/buzzy/`** (matches plugin convention, invisible by design), or (b) use a visible sibling like `notes.md.buzzy` if buzzy should be filesystem-visible outside Obsidian.

2. **Prefer per-vault directory over per-file sidecars.** Both tools reserve one config dir (`.obsidian/`, `logseq/`). A single `.obsidian/plugins/buzzy/state/<hash>.bin` per file scales better than N scattered sidecars and interacts predictably with existing sync tools.

3. **Do not embed CRDT bytes inline in `.md` files.** No prior art does this. Obsidian's frontmatter is typed and small; large binary blobs break the properties UI. Logseq's `key:: value` is single-line-only. Base64-encoded Yjs updates in frontmatter would collide with users editing frontmatter manually and produce enormous git diffs.

4. **Logseq's `id:: <uuid>` is a strong pattern for stable block identity in plain text.** If buzzy needs per-block anchors surviving line-based edits, an inline `<!-- buzzy-id: <uuid> -->` HTML comment is a portable equivalent (comments render empty in preview; Logseq preserves them).

5. **Prior CRDT work uses Yjs, not Automerge, in the Obsidian ecosystem.** Relay (Yjs + y-sweet) and likely Peerdraft chose Yjs. Neither persists CRDT state as visible sidecar files; state lives server-side or in memory. buzzy, being a **daemon with local persistence**, is the novel piece — the sidecar-on-disk problem is unsolved.

6. **Assume users run multiple sync tools concurrently.** obsidian-livesync explicitly warns against running alongside Obsidian Sync. buzzy's on-disk state must be idempotent-safe under rsync/Dropbox/iCloud (which may reorder writes or partially replicate files). A single per-vault state file is easier to reason about than N per-file sidecars.

7. **Workspace churn is real.** `.obsidian/workspace.json` rewrites on every focus event; docs tell users to gitignore it. buzzy should not put frequently-updated state in a git-tracked location by default.

---

## 5. Markdown standards and metadata conventions

### 5.1 Markdown flavors

| Flavor | Frontmatter | `{attr}` syntax | `[[wiki]]` | Raw HTML | Tracked changes |
|---|---|---|---|---|---|
| CommonMark | Not in spec | Not in spec | Not in spec | Yes (blocks + inline) | No |
| GFM | Not in spec | Not in spec | Not in spec | Yes, sanitized | No |
| MultiMarkdown | `Key: Value` (— optional) | No | No | Via `HTML Header` | No |
| Pandoc | `yaml_metadata_block` extension | Yes (native) | Extension only | Yes + `{=format}` raw blocks | docx-import only |
| Obsidian | YAML `---` (Properties) | Plugin-only | Yes, native | Yes | No |

Only Pandoc treats attribute syntax and typed metadata as first-class parser features. YAML frontmatter is a downstream convention adopted by Jekyll, Hugo, Obsidian, and Pandoc — but neither CommonMark ([spec.commonmark.org/0.31.2](https://spec.commonmark.org/0.31.2/)) nor GFM ([github.github.com/gfm](https://github.github.com/gfm/)) standardises it. Pandoc's `--track-changes` reads Word tracked changes on docx import and emits pandoc-AST spans with classes `insertion` / `deletion` / `comment-start` / `comment-end` — this is not Markdown-native syntax.

**No mainstream flavor defines native tracked-change or CRDT syntax.** Collaboration is universally handled outside the document format.

### 5.2 YAML frontmatter conventions

**No IANA, W3C, or RFC-registered vocabulary exists** for markdown frontmatter. The MIME type `application/yaml` was finalised in 2024, but frontmatter conventions remain de facto only.

**Keys with cross-tool traction (3+ tools):**

- **`title`** — near-universal (Jekyll, Hugo, Zola, Astro, Obsidian, Foam, Dendron, Pandoc). Strongest de facto convention.
- **`tags`** — Jekyll, Hugo, Astro, Obsidian, Foam, Dendron, Logseq. Pandoc uses `keywords`.
- **`aliases`** (plural) — Hugo, Zola, Obsidian, Foam. Logseq uses `alias` (singular). Clearest cross-tool convention outside `title`.
- **`date`** — Jekyll/Hugo/Zola/Pandoc converge on `date`; Astro splits into `pubDate`/`updatedDate`; Dendron uses `created`/`updated` (epoch ms). Formats span YYYY-MM-DD, RFC 3339, and epoch — no format is universal.
- **`description`** — Hugo, Zola, Astro, Obsidian Publish, Pandoc.
- **`draft` / `published`** — Hugo, Zola, Astro, Jekyll.

**No cross-tool `id`/`uid` convention.** Only Dendron and Logseq use one and their semantics differ. Bear uses inline `#tags` (no frontmatter). Notion's markdown export has no standardised frontmatter.

Closest formal mapping: [schema.org/CreativeWork](https://schema.org/CreativeWork). Hugo and Astro examples often shadow these but no tool mandates the mapping.

**Recommendation for buzzy:** if buzzy needs a document ID in the file itself, use a namespaced key like `buzzy_doc_id` or `buzzy: {id: ..., version: ...}` to avoid collision.

### 5.3 Collaboration-metadata standards — the null result

**No formal cross-vendor standard (RFC / W3C / ISO) exists for embedding tracked changes, comments, or CRDT operations in Markdown or plaintext.**

- **CriticMarkup** ([criticmarkup.com](https://criticmarkup.com), 2013): informal convention, not RFC/W3C. Syntax: `{++add++}`, `{--del--}`, `{~~old~>new~~}`, `{>>comment<<}`, `{==highlight==}`. Adopted by Marked, iA Writer, Sublime Text, BBEdit, TextMate, PanDiff. De facto within the Markdown power-user niche; no governance body. **The closest thing to a shared convention for inline change-tracking in Markdown.**
- **Pandoc `--track-changes`**: one-tool output. Not a standard.
- **reStructuredText / AsciiDoc**: only line comments (`..`, `//`); no tracked-changes syntax.
- **W3C Web Annotation Data Model** ([w3.org/TR/annotation-model](https://www.w3.org/TR/annotation-model/), 2017 Recommendation): the only real W3C standard nearby. Defines `TextQuoteSelector` (exact + prefix + suffix), `TextPositionSelector` (char offsets), and RFC 5147 fragment selectors for `text/plain`. Media-type-agnostic, so applies to Markdown in principle. But it's an external JSON-LD sidecar model; transport deferred to the Annotation Protocol, and silent on CRDTs and real-time collaboration.
- **IETF**: a [datatracker search for "CRDT"](https://datatracker.ietf.org/doc/search?name=CRDT) returns zero drafts or RFCs.
- **Automerge**: binary chunks in a pluggable storage backend keyed by `[docID, chunkType, chunkID]`. Replaces plaintext rather than sitting alongside it.
- **Yjs**: wire/persistence format in `github.com/yjs/y-protocols`; project-maintained, not standardised.
- **HedgeDoc/CodiMD**: no documented plaintext-embedded CRDT format.

**Bottom line: there is no CRDT sidecar standard.** buzzy defines new ground.

### 5.4 Sidecar file patterns

**Real standards:**

- **XMP** ([ISO 16684-1:2019](https://en.wikipedia.org/wiki/Extensible_Metadata_Platform)): embedded in JPEG/PDF/TIFF/PSD; as `filename.xmp` sidecar for DNG. Plain XML — syncs cleanly through Dropbox/iCloud/git.

**De facto conventions that travel well:** `.srt` (subtitles), `.pp3` (RawTherapee), `.dop` (DxO), `.cos` (Capture One), `.tfw`/`.jgw` (geo-registration world files). Basename-match, tool-owned.

**Historical per-file VCS sidecars:** RCS `foo.rb,v`, SCCS `s.foo.c`. Fossil went the opposite direction (single SQLite for whole repo). Git consolidates into `.git/` — never per-file.

**OS-generated per-directory noise (bad sync citizens):**

- `.DS_Store` (macOS Finder, changes constantly, causes Google Drive copyright false-positives, retroactively cancels multi-file transfers on collision)
- `Thumbs.db`, `desktop.ini` (Windows)

**Apple metadata specifically:** macOS xattrs (`com.apple.quarantine`, `com.apple.metadata:*`) do not survive git (stripped), historically stripped by Dropbox, partially preserved by iCloud, encoded as **AppleDouble `._filename`** on FAT/exFAT/SMB/NFS/WebDAV — which then become visible clutter on Windows/Linux. `__MACOSX/` folder inside Finder-made zips is another leak. `dot_clean -m` merges them back.

**Critical finding:** **no widely-adopted CRDT tool shadows a plaintext document with a per-file sidecar.** Both Automerge (via `automerge-repo` → IndexedDB / nodefs adapters keyed by `[docID, chunkType, chunkID]`) and Yjs (via `y-indexeddb`, keyed by docName) persist to opaque binary stores, not filename-linked sidecars. buzzy fills a genuine gap.

### 5.5 Sidecar naming — interop matrix

Assume `foo.md` is the primary file. Windows Explorer ignores the dotfile convention entirely — `.foo.md` and `.buzzy/` are fully visible on Windows. Cross-platform hiddenness is *not* a property of the leading dot.

| Pattern | Obsidian | git | Dropbox | iCloud | GDrive | Finder / Explorer | Precedent |
|---|---|---|---|---|---|---|---|
| `foo.md.meta` | Visible as phantom doc | Tracked | Syncs | Syncs | Syncs | Fully visible, user-deletable | Unity per-asset meta |
| `.foo.md` | Silently hidden | Tracked | Syncs | Syncs (hidden in Finder) | Syncs | Hidden macOS / visible Windows | Unix; vim swap |
| `foo.md.crdt` | Visible phantom | Tracked | Syncs | Syncs | Syncs | Visible | None established |
| `.buzzy/foo.md` | Directory hidden | Tracked (gotchas) | Syncs | Syncs | Syncs | Hidden macOS / visible Windows | git, hg, cargo — dominant |
| `foo/.foo.md` | Parent visible, inner hidden | Tracked | Syncs | Syncs | Syncs | Mixed | No well-known example |
| `foo.md.xmp` | Visible | Tracked | Syncs | Syncs | Syncs | Visible | Adobe XMP / ExifTool |
| `.foo.md.swp` | Hidden | Needs `*.swp` gitignore | Syncs | Syncs | Syncs | Hidden macOS / visible Win | vim |

Key findings:

1. **No RFC exists for sidecar naming.** Tools split roughly 50/50 between "replace extension" (`foo.xmp`) and "append extension" (`foo.md.meta`). Append is collision-safer.
2. **Obsidian silently hides all dot-prefixed files and directories.** The `obsidian-show-dotfiles` plugin exists precisely because of this.
3. **Dropbox is the most sidecar-friendly cloud sync:** narrow denylist (`.DS_Store`, `desktop.ini`, `thumbs.db`, `.dropbox*`, `~$*`, `.~*`); arbitrary dotfiles sync fine.
4. **iCloud syncs dotfiles** but Apple documents no positive guarantee.
5. **git's trailing-slash pattern is a footgun** — `.buzzy` vs `.buzzy/` behave differently, and once a parent is excluded, `!.buzzy/keep` re-includes fail.
6. **Windows Explorer ignores dotfile hiddenness entirely.**

---

## 6. Automerge vs Yjs — on-disk formats

Research date: 2026-07-06.

### 6.1 Automerge on-disk format

**Versions.** Automerge JS latest: `3.4.0-rev-frag-hex.3` (2026-07-02); Automerge 3 line released July 2025. Rust crate `automerge` on crates.io: **`0.10.0`**. Automerge 3 uses the same file format as Automerge 2 — format is stable across the 2→3 transition.

**Magic bytes** (from `rust/automerge/src/storage.rs`):

```rust
pub(crate) const MAGIC_BYTES: [u8; 4] = [0x85, 0x6f, 0x4a, 0x83];
```

**Chunk header** (from `rust/automerge/src/storage/chunk.rs`):

| Offset | Field | Size | Notes |
|---|---|---|---|
| 0 | Magic | 4 B | `85 6F 4A 83` |
| 4 | Checksum | 4 B | First 4 bytes of SHA-256 over `chunk_type_byte ‖ uleb128(data_len) ‖ data` |
| 8 | Chunk type | 1 B | `0x00`=Document, `0x01`=Change, `0x02`=Compressed, `0x03`=Bundle |
| 9 | Data length | ULEB128 | Variable |
| … | Chunk data | data_len | Columnar body |

**Body encoding.** Columnar store: prefix (actors, heads, change/ops metadata), change columns, ops columns, head-index suffix. Columns individually compressible via `CompressConfig::Threshold`; LEB128 varints for counts and index tables.

**Size.** Automerge 3 rearchitected memory — pasting *Moby Dick* went from ~700 MB in-memory (v2) to ~1.3 MB (v3). **On-disk numbers for a specific op count are not published in the primary sources.**

**Inspection / CLI.** `rust/automerge-cli` provides `export`, `import`, `examine`, `examine-sync`, `merge`. `examine` "reads an automerge document and prints a JSON representation of the changes in it to stdout."

**File extension.** No canonical extension documented in primary sources. `.automerge` is community usage.

### 6.2 Yjs on-disk format

**Versions.** Yjs latest: `v14.0.0-rc.23` (2026-07-05). README highlights `v13.6.31` (2026-05-28) — 13.6.x is the last stable minor before 14.x-rc. `yrs` (Rust) latest on crates.io: **`0.27.2`** (2026-06-12).

**Byte layout — mostly undocumented.** Yjs's own docs are silent on byte-level layout. `docs.yjs.dev/api/document-updates` describes updates only conceptually: `Uint8Array`, "binary encoded (highly compressed)", commutative/associative/idempotent. `INTERNALS.md` describes the *object model* (Item structure, `ID(clientID, clock)` Lamport pair, 53-bit `clientID`) but **no magic bytes, no field-level wire layout, no named encoders** — it defers to `src/structs/Item.js` and y-protocols. Notable contrast with Automerge, whose format has published constants.

**v1 vs v2.** v1 default (`encodeStateAsUpdate`, `update` event); v2 (`encodeStateAsUpdateV2`, `updateV2` event) is "up to 10x more efficient" but still marked experimental on `docs.yjs.dev/api/y.doc`. Converters exist (`Y.convertUpdateFormatV1ToV2` / `V2ToV1`). No file magic.

**Merge/diff/GC.** `Y.mergeUpdates([Uint8Array])` deduplicates but "doesn't garbage-collect deleted content. You still need to load the document to a Y.Doc to reduce the document size." `Y.diffUpdate(update, stateVector)` computes a delta without a live Y.Doc.

**Inspection.** `Y.logUpdate(Uint8Array)` (experimental). Third-party tools: `inspector.yjs.dev`, y-sweet debugger, liveblocks devtools. **No CLI in the yjs repo.** `Y.obfuscateUpdate(update)` scrubs content, keeps structure.

### 6.3 Rich text / markdown handling

**Automerge 3.** Previous `Text` class removed; ordinary JS `string` values are the collaborative text type; `@automerge/automerge/next` is now the default namespace. `RawString` renamed `ImmutableString`. Per Ink & Switch's Peritext paper, Automerge does **not** implement Peritext natively — the Peritext prototype "extends a simplified version of the Automerge CRDT library" and integration back into Automerge remains future work.

**Yjs.** `Y.Text` with `insert`, `delete`, `format`, `applyDelta`, `toDelta`. Delta format = Quill's Delta. Ink & Switch describes Yjs as *"the most full-featured rich-text CRDT available today"* but notes concurrent-formatting anomalies via the control-character approach. Editor bindings: ProseMirror, Quill, CodeMirror, Monaco, Slate, Tiptap, Milkdown, Lexical, BlockNote. No `y-markdown` package in the Yjs README.

**Neither ships a markdown CRDT.** Both expect the app to model formatting via marks/attributes.

### 6.4 Storage adapter models

**automerge-repo** (`v2.5.6` stable; `v2.6.0-alpha.2` on 2026-06-05). `StorageAdapter` treats values as opaque `Uint8Array`s keyed by:

```
key = [<document ID>, <chunk type>, <chunk identifier>]
```

`chunk type` = `"snapshot"` (compacted) or `"incremental"` (single change / change set); chunk id = doc heads at compaction, or SHA-256 of change bytes. Example: `["3RFyJzsLsZ7MsbG98rcuZ4FqtGW7", "incremental", "0290cdc2..."]`. Adapters: **IndexedDB** and **Node FS** (`@automerge/automerge-repo-storage-indexeddb`, `@automerge/automerge-repo-storage-nodefs`). Append-only chunks compacted opportunistically into snapshots.

**y-leveldb** (`v0.2.0`, 2025-04-23, **repo archived/deprecated**). "Incremental updates" internally with `flushDocument` "(dev only)" compaction. Public API: `storeUpdate(docName, update)`, `getYDoc(docName)`; state vectors stored separately.

**y-indexeddb** (`v9.0.12`, 2023-11-02). Emits `synced` event when hydration completes. Append-vs-blob strategy not documented.

### 6.5 External-file-edit reconciliation — the decisive finding

**Automerge has direct support:** `rust/automerge/src/text_diff.rs` implements Myers diff over Unicode graphemes and translates the edit script into `tx.splice_text` / `tx.delete` / block ops via a `DiffHook` trait:

- `myers_diff(doc, tx, patch_log, text_obj, new)` — plain-text reconciliation.
- `myers_block_diff(doc, tx, patch_log, text_obj, new, config)` — reconciles text and block structure, plus `apply_marks_diff` with per-mark expand policy.

Aligns edits to grapheme boundaries via `unicode_segmentation`, advances an index cursor in the doc's `TextEncoding` (UTF-8/UTF-16/graphemes) when emitting ops. **This is exactly the "file modified externally → synthetic CRDT ops" pattern buzzy needs, shipped in the reference Rust implementation.** Entry points are `pub(crate)`; public API is via `Transaction::update_text` / `update_spans`.

**Yjs has no equivalent shipped.** `Y.Text` exposes `insert` / `delete` / `format` / `applyDelta`. Consumers must compute a diff externally and translate to ops — a normal pattern in Yjs editor bindings (`y-codemirror`, `y-monaco`) but not packaged as a reusable helper.

**Prior art scan.** `jupyter-collaboration / jupyter_ydoc` defines CRDT *schemas* (`YFile`, `YNotebook`) but reconciliation logic lives in `jupyter-server-ydoc`, not documented in the reachable READMEs. Logseq DB version uses SQLite (not markdown files) + alpha RTC. Obsidian Sync proprietary. `gh search repos "crdt file sync markdown"` returned zero results.

### 6.6 Portability and Rust bindings

**Automerge Rust story is native.** Core is Rust (`automerge` `0.10.0`); JavaScript is a WASM wrapper. Published bindings per top-level README: **JavaScript (WASM), Rust, C (`automerge-c`), Deno**. **A Rust daemon can use `automerge` directly with no FFI.**

**Yjs core is JS.** Rust port is `y-crdt`/`yrs`, aiming for "behavior and binary protocol compatibility with Yjs." Sub-crates: `yrs` (Rust, `0.27.2`), `yffi` (C FFI), `ywasm` (WASM). Downstream: `pycrdt`, `yswift`, `ydotnet`, `yrb`, `ykt`, `yr`. `yrs 0.21` listed in feature-parity table — table lags releases.

**Inspection outside JS.** Automerge: `automerge examine` CLI prints JSON. Yjs: **no non-JS inspector found in primary sources**; `inspector.yjs.dev` and `Y.logUpdate` are JS-only; yrs ships no CLI.

### 6.7 Recommendation matrix for the buzzy sidecar

| Requirement | Choice | Why |
|---|---|---|
| Inspectability from shell | **Automerge** | `automerge examine file.automerge` → JSON. Magic bytes `[85 6F 4A 83]`, SHA-256 checksum prefix, chunk-type discriminator make files self-identifying. Yjs updates have no magic, no CLI. |
| Smallest on-disk size | **Yjs v2** (probably) | v2 is "up to 10x more efficient" (README) but still experimental. No head-to-head byte-size benchmark from primary sources. |
| Rust daemon, first-class native | **Automerge** | Core *is* Rust; JS is the wrapper. Yjs core is JS; Rust access is via yrs, a parallel implementation aiming for protocol compatibility, not the reference. |
| Best rich-text semantics | **Yjs (today)** by Ink & Switch's assessment; **Automerge (future)** if Peritext is integrated | Yjs has "the most full-featured rich-text CRDT available today" but concurrent-formatting anomalies; Automerge team plans to integrate Peritext. Neither ships a markdown-native CRDT. |
| External-edit reconciliation (the buzzy core) | **Automerge** | `text_diff.rs` ships Myers-diff → splice/delete/mark ops out of the box, in Rust. Yjs requires app-level diff → `Y.Text` op translation. |
| Storage layout for a `.crdt` sidecar (one file) | **Automerge core lib** (not automerge-repo) | Core lib's `Automerge.save()` produces a single self-contained blob with magic bytes; automerge-repo splits into `snapshot`+`incremental` chunks keyed by tuples, doesn't map to one file. |
| Forward compatibility | **Automerge** | Spec mandates unknown columns, value tags, action codes MUST be retained through read-write cycles. Sidecar written by daemon vN survives vN+1 read-write without lossy conversion. |

### 6.8 Open questions from the Automerge/Yjs pass

1. Automerge file extension — no canonical extension documented. `.automerge` is community usage.
2. Published on-disk-size numbers — neither project publishes "N ops → M KB" tables. Automerge 3 headlines are runtime memory, not disk size.
3. Whether `text_diff::myers_diff` is on the stable public `automerge` crate API — module marks entry points `pub(crate)`; public surface likely calls it via `Transaction::update_text` / `update_spans`, but that specific mapping wasn't visible from the file alone.
4. automerge-repo NodeFS directory layout — key tuple `[docId, chunkType, chunkId]` documented, but how NodeFS materialises tuples as files/dirs is not.
5. y-indexeddb append-vs-blob strategy — README documents only public API.
6. Yjs byte-level wire format — `docs.yjs.dev` and `INTERNALS.md` describe object model, not byte layout. Real spec lives in `y-protocols` and lib0 source.
7. Automerge Peritext status in shipping 3.x — 2021 Peritext paper says future work; `automerge-3` blog doesn't mention Peritext; `/docs/documents/rich_text/` URL 404s currently.
8. Prior art for CRDT-plus-plain-file markdown sync — Jupyter is closest published example but reconciliation logic wasn't in reachable READMEs.

---

## 7. Answers to the four buzzy questions

### Q1: What format should the sidecar file use?

**Recommendation: Automerge core (Rust crate `automerge` 0.10.x), raw binary, no wrapper.**

Decisive finding: Automerge ships a Myers-diff-to-CRDT-ops implementation (`rust/automerge/src/text_diff.rs`) in Rust that directly solves buzzy's core problem — vim writes a modified file, produce synthetic ops. Yjs has no equivalent shipped; consumers must build the diff-to-ops step themselves.

Additional Automerge advantages for the sidecar use case:

- Self-describing binary — magic bytes `[85 6F 4A 83]`, SHA-256 checksum prefix, chunk-type byte make files identifiable and validatable without library knowledge.
- CLI inspection via `automerge examine` — dumps to JSON.
- Rust-native — core *is* Rust; JS is the wrapper. If buzzy is a Rust daemon, no FFI.
- Forward compatibility guaranteed by spec — unknown columns/value tags/action codes MUST be retained through read-write cycles. No need for a version tag in the sidecar.
- Complete history retained by design — aligns with buzzy's per-document capability grants and permission-at-merge-time model.
- `Automerge.save()` / `Automerge.load()` / `Automerge.saveIncremental()` produces a single self-contained blob per doc — the right shape for one-`.md`-one-`.crdt`. **Do not use automerge-repo's storage adapter model** for `.crdt`; it splits into `snapshot`+`incremental` chunks keyed by tuples, which is the wrong shape for the sidecar.

**Trade-off to accept:** Automerge's rich-text formatting semantics are, per Ink & Switch, weaker than Yjs's today. For buzzy's markdown-canonical model — where formatting lives *in the text* (asterisks for bold, hashes for headings), not in a separate marks layer — this trade-off is minimal. Text CRDT correctness dominates over rich-text mark correctness.

**Ecosystem counterweight:** the Obsidian collab ecosystem uses Yjs (Relay, Peerdraft; AFFiNE and Outline also Yjs). Choosing Automerge means buzzy is off the Obsidian-collab default rail. Mitigation: buzzy is a daemon, not an editor plugin, so it has no direct dependency on the Yjs editor bindings ecosystem. The Rust story is where the ecosystem is *weakest* on the Yjs side (`yrs` is a compat implementation, not the reference).

### Q2: How to maintain the invariant that `.md` is always valid and readable without the sidecar?

**No surveyed tool enforces this.** AFFiNE, Outline, Upwelling, Logseq DB-version, HedgeDoc all treat CRDT state as canonical and markdown as lossy export. buzzy's contract is genuinely novel.

Concrete implementation obligations:

1. **Every daemon commit: render CRDT → markdown → write `.md` atomically (temp + rename), *then* write sidecar.** Never write sidecar before `.md`; that risks the invariant if the daemon crashes between writes.

2. **Constrain the block schema to a lossless-round-trip subset of GFM.** Outline's cautionary tale: their schema grew comment marks, highlight colours, table `colwidth` — none representable in markdown. Their `mergeAttrs` helper exists specifically to survive round-trip; their JSDoc calls markdown export "lossy". buzzy cannot afford this — a user opening the file in vim would lose data.

3. **On daemon startup: verify `sidecar-rendered-text == .md-on-disk`.** If divergent, treat `.md` as newer (offline external edit) and run the diff-to-ops step from Q3.

4. **Peritext's guidance directly applies:** keep formatting marks as *separate* CRDT ops keyed on character opIds. Do not embed formatting inline in the CRDT text sequence. Peritext explicitly rejects "markdown-in-plaintext-CRDT" — concurrent bold produces `**The **fox** jumped.**`. Follow Peritext's split: plaintext CRDT sequence + mark ops referencing character opIds.

5. **Logseq's `id:: <uuid>` inline convention is stable prior art for block anchors that survive line-based edits.** For buzzy's comment-anchor stability, an HTML-comment variant like `<!-- buzzy: block=<uuid> -->` renders empty in Obsidian/Logseq preview and is preserved by both. Consider as anchoring fallback when Peritext's opId-based approach can't be applied.

### Q3: How to handle file-modified-externally?

Only Outline demonstrates a working reconciliation pattern in production, and only for structured (ProseMirror JSON) external edits. For plaintext external edits — buzzy's actual case — the primary art is:

- **Automerge's `text_diff.rs`** — the mechanism, shipping.
- **Outline's `APIUpdateExtension.ts`** — the *state-vector-diff-then-`applyUpdate`* pattern for propagating an externally-computed change into a live CRDT session.

Concrete buzzy plan:

1. FS watcher (fsevents / inotify) detects `.md` write.
2. Debounce 100–500 ms (batch rapid saves from editors like vim).
3. Read file. Render current CRDT to text (call this "expected"). Read file contents ("actual").
4. If `expected == actual`, do nothing (write was our own).
5. Else: call `Transaction::update_text` on the Automerge doc with `actual`. This invokes `myers_diff` internally and emits splice ops with a synthetic actor ID marked `external:$editor`.
6. Validate block tree post-apply. If it went non-well-formed (unbalanced code fence, broken table), revert the `.md` from CRDT-rendered text and surface the error to the user.
7. Broadcast the incremental change to peers via `saveIncremental()`.

Comment anchors survive because Peritext's opId+qualifier scheme preserves them across text edits, including deletions (tombstones outlive characters).

**Open risk to design around:** if the external edit is *destructive* (user deletes half the doc in vim), the diff produces a large delete op that peers cannot distinguish from an intentional deletion. Warn on suspiciously-large deltas and require confirmation, or auto-save a snapshot to the CRDT history before applying.

### Q4: Any standards for "plain text file has CRDT metadata attached"?

**Confirmed null result:** no formal cross-vendor standard (RFC / W3C / ISO) for embedding tracked changes, comments, or CRDT operations in Markdown or plaintext. IETF datatracker returns zero results for "CRDT".

**Closest existing neighbours worth aligning with:**

- **W3C Web Annotation Data Model** (2017 Recommendation) — `TextQuoteSelector` (exact + prefix + suffix) and `TextPositionSelector` (char offsets) apply to `text/plain` and by extension to Markdown. Recommend buzzy's comment-anchor JSON schema mirror these selectors — free interop with any tool that already speaks W3C Annotation.
- **CriticMarkup** — one-tool convention, adopted by ~6 markdown editors. Closest thing to a shared change-tracking convention in Markdown. Recommend as the interop target if buzzy emits inline suggestion markers.
- **XMP (ISO 16684-1)** — closest *concept* (external sidecar for metadata) but for images, not text. Naming precedent for the "sidecar file" idea.

**Positioning claim for the design doc:** buzzy proposes the interop protocol that Upwelling (Ink & Switch 2023) explicitly asked for — quoting their essay: *"a file format or exchange protocol that makes it possible for writers to use the writing software of their choice."*

---

## 8. Consolidated "Alternatives Considered" table for the design doc

| Alternative | Storage | Real-time collab? | Markdown canonical? | Rejected because |
|---|---|---|---|---|
| **AFFiNE / BlockSuite** | Yjs binary in SQLite/IndexedDB/Postgres | Yes (WebSocket / WebRTC providers) | No (JSON snapshots → adapter → md) | CRDT-canonical, markdown as lossy export. Gives up on external-editor interop. |
| **Outline (Hocuspocus + y-prosemirror)** | Yjs binary + ProseMirror JSON in Postgres | Yes (WebSocket) | No (`text` column deprecated; JSDoc: "lossy") | Server-first. No local file model. Explicitly moved *away* from markdown-canonical. |
| **Upwelling (Ink & Switch)** | Automerge tarball | Yes (Automerge protocol) | No | CRDT-only. Authors themselves note they wrote the essay in Google Docs + git because the tool doesn't yet solve the interop problem. |
| **Obsidian Sync (proprietary)** | Undocumented; behaviour suggests snapshot-based | Near-real-time file sync | Yes (`.md` on disk) | Not CRDT. Multi-writer conflicts unresolved. |
| **obsidian-livesync** | PouchDB / CouchDB `_changes` feed | Near-real-time | Yes | Not CRDT — CouchDB revision tree. README warns against running alongside Obsidian Sync. |
| **Relay (System 3)** | Yjs, server-side | Yes | Yes (`.md` on disk) | State held server-side; no local persistence. Fails "network is optional" ideal. |
| **CRDT sidecar (chosen approach)** | Automerge binary sidecar per doc | Yes (daemon-mediated) | Yes | **This is buzzy.** |

---

## 9. Ecosystem lessons distilled

1. **Nobody has shipped CRDT-persisted-alongside-plain-markdown-file before buzzy.** Every surveyed tool either goes CRDT-canonical (AFFiNE, Outline, Upwelling) or non-CRDT-sync (Obsidian Sync, obsidian-livesync) or server-only-CRDT (Relay). buzzy's niche is genuinely empty.

2. **Peritext is required reading** — it defines the anchor mechanism (opIds + before/after qualifiers + tombstones) that keeps comments stable across concurrent edits, and it explicitly rules out simpler approaches (markdown-in-plaintext-CRDT). Cite it in the design doc's comment-anchoring section.

3. **File layout — Option B recommended for MVP.** Store all CRDT state under `.obsidian/plugins/buzzy/state/<sha256-of-vault-relative-path>.bin`. Reasons: (a) Obsidian's sanctioned plugin storage location, invisible in UI on all platforms; (b) syncs cleanly through Dropbox/iCloud/git; (c) no per-file clutter (a 10k-note vault × 3 sidecars = 30k phantom files under the current sibling-file design); (d) idempotent-safe under rsync/iCloud partial replication. Keep `.buzzy/` at vault root as the future editor-agnostic layout (Windows Explorer shows it, but same caveat as `.git/`).

4. **CriticMarkup + W3C Annotation as interop targets.** Emitting these formats where possible gives buzzy a story for tools that don't run the daemon (a user opens the file in a CriticMarkup-aware editor; changes appear as reviewable suggestions).

5. **The current design doc's `File Format` section (`buzzy_design_doc.md:182-192`) should be reworked.** The current visible-sibling layout (`meeting-notes.md.crdt`, `.access`, `.keys`) works correctly but scatters state across N files per document, which is fragile under partial-sync (Dropbox, iCloud), invites accidental deletion, and produces clutter at vault scale. Option B (centralised `.obsidian/plugins/buzzy/state/`) is the recommended MVP layout.
