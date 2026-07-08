//! Architecture as a test. Enforces the dependency-inversion boundary: a concrete
//! CRDT engine (`automerge`) must be reachable ONLY from `buzzy-crdt-automerge`.
//! Runs as part of `cargo test --workspace`, so a commit that wires the graph
//! wrong turns the normal test suite red.
//!
//! In Phase 0 `automerge` is not yet a dependency anywhere, so this passes
//! trivially; it becomes load-bearing the moment Phase 1 adds the crate.
#![allow(clippy::unwrap_used)]

use cargo_metadata::{Metadata, MetadataCommand, PackageId};
use std::collections::{HashMap, HashSet};

/// Crates that must never reach `automerge` through their dependency graph.
const QUARANTINED_FROM: &[&str] = &[
    "buzzy-crdt",
    "buzzy-protocol",
    "buzzy-net",
    "buzzy-cli",
    "buzzy-crdt-mock",
];

const FORBIDDEN: &str = "automerge";

#[test]
fn concrete_engine_stays_quarantined() {
    let meta = MetadataCommand::new().exec().unwrap();
    for crate_name in QUARANTINED_FROM {
        assert!(
            !reaches(&meta, crate_name, FORBIDDEN),
            "boundary violation: `{crate_name}` transitively depends on `{FORBIDDEN}`; \
             a concrete CRDT engine must only be reachable from `buzzy-crdt-automerge` \
             (composition happens in buzzy-daemon/src/main.rs)"
        );
    }
}

/// Does `from_name` transitively depend on a package named `target`?
fn reaches(meta: &Metadata, from_name: &str, target: &str) -> bool {
    let Some(resolve) = meta.resolve.as_ref() else {
        return false;
    };
    let nodes: HashMap<&PackageId, _> = resolve.nodes.iter().map(|n| (&n.id, n)).collect();
    let name_of: HashMap<&PackageId, &str> = meta
        .packages
        .iter()
        .map(|p| (&p.id, p.name.as_str()))
        .collect();

    let Some(start) = meta
        .packages
        .iter()
        .find(|p| p.name == from_name)
        .map(|p| &p.id)
    else {
        return false;
    };

    let mut seen: HashSet<&PackageId> = HashSet::new();
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        if let Some(node) = nodes.get(id) {
            for dep in &node.dependencies {
                if name_of.get(dep).copied() == Some(target) {
                    return true;
                }
                stack.push(dep);
            }
        }
    }
    false
}
