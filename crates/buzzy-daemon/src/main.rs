//! buzd — the Buzzy daemon.
//!
//! This file is the **composition root**: the one and only place that names a
//! concrete CRDT engine. Every other module works through the `buzzy-crdt` traits.
//! Swapping engines (Automerge, Loro, yrs) is a one-line change here.

use buzzy_crdt::CrdtEngine;
use buzzy_crdt_mock::MockEngine;

fn main() {
    // ── Engine selection (the ONLY engine-aware line in the daemon) ──
    // Phase 1: replace with `buzzy_crdt_automerge::AutomergeEngine::new()`.
    let engine = MockEngine::new();

    match engine.create_document("") {
        Ok(_doc) => {
            println!("buzd 0.1.0 (skeleton) — engine initialised, socket server not yet wired")
        }
        Err(e) => eprintln!("buzd: failed to initialise engine: {e}"),
    }
}
