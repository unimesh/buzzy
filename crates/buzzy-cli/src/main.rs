//! bzz — the Buzzy CLI. Talks to a running `buzd` over its Unix socket (Phase 2).
//! Phase 0 is a command-dispatch skeleton.

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "status" => println!("bzz: daemon not reachable yet (skeleton)"),
        "" | "help" | "--help" | "-h" => {
            println!("usage: bzz <init|start|stop|status|share|peers|log|pause|resume>");
        }
        other => println!("bzz: unknown command {other:?} — run `bzz help`"),
    }
}
