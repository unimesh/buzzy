# Peer Discovery and Decentralized Identity — Research for buzzy

Technical survey of how local-first and P2P software handles peer discovery, identity, NAT traversal, and key management without central servers. Organized in five parts, followed by consolidated recommendations for the buzzy protocol.

- **Scope**: mDNS/DHT/ICE for discovery; Ed25519/DIDs/petnames/OIDC/TOFU for identity; Syncthing/Signal/Matrix/Iroh/Tailscale/IPFS deep-dive; NAT taxonomy through DERP and MASQUE; key-management UX from Signal-style invisibility through BIP39.
- **Audience**: buzzy protocol designers. Developer-first phase, non-technical users eventually. Must support "one user, multiple devices" (Alice = laptop + desktop).
- **Companion file**: `/Users/unnimesh/Obsidian/Design/assets/buzzy/nat-traversal-analysis.md` — the raw NAT-traversal reference with RFC citations, benchmarks, and CVE history.

---

## Part 1 — Peer Discovery Mechanisms

Building a local-first sync protocol requires layering multiple discovery mechanisms because no single approach handles LAN, WAN, mobile, and adversarial network conditions well.

### mDNS / DNS-SD (Bonjour)

RFC 6762 (mDNS) and RFC 6763 (DNS-SD) provide zero-configuration discovery on a single link. Clients send DNS queries to the multicast group `224.0.0.251` (IPv6 `ff02::fb`) on UDP port 5353; hosts owning the queried name respond directly. Service types follow `_service._proto.local` with `PTR` records enumerating instances, `SRV` records giving host:port, and `TXT` records carrying key-value metadata.

Concrete deployments:
- **Syncthing** advertises `_syncthing._tcp.local` with a `TXT` record containing the Device ID (a truncated SHA-256 of the TLS certificate).
- **Chromecast** uses `_googlecast._tcp`, **HomeKit** `_hap._tcp`, **AirPlay** `_airplay._tcp` and `_raop._tcp`.
- **AirDrop** layers `_airdrop._tcp` over Apple Wireless Direct Link (AWDL), a peer-to-peer WiFi variant that doesn't require a common network.

Failure modes are significant in the environments this mechanism was designed for. Enterprise WiFi commonly blocks multicast to reduce airtime overhead; guest and hotel WiFi enable client isolation (also called AP isolation), which drops peer-to-peer traffic entirely. mDNS does not cross subnets without a reflector (Avahi's `reflector=yes`, or specialized proxies), which most consumer routers do not run. IPv6 link-local scoping adds interface-selection complexity on multi-homed hosts. Windows had inconsistent mDNS support before Windows 10 build 1803 shipped a native resolver.

### DHT (Distributed Hash Table)

Kademlia (Maymounkov & Mazières, 2002) is the dominant DHT design. Each node has an ID in the same keyspace as content keys; distance is the XOR of two IDs. Nodes maintain `k`-buckets (typically k=8 or k=20) of known peers at each distance bracket, and lookups proceed iteratively: query the α closest known nodes (α=3), take the closest results they return, repeat until convergence. Lookup complexity is O(log N).

- **BitTorrent Mainline DHT** (BEP 5) has held 10-25 million concurrent peers historically and remains the largest deployed DHT.
- **libp2p Kademlia** underpins IPFS, where the node ID is derived from the peer's Ed25519 or secp256k1 public key (typically as a multihash, historically SHA-256).
- Bootstrap is via a hardcoded list of well-known nodes (e.g., `bootstrap.libp2p.io`); once joined, the DHT is self-sustaining.

Trade-offs are considerable. First-lookup latency is often 2-10 seconds for cold caches; IPFS has historically struggled with content resolution times, which drove the introduction of `ipfs-bitswap` sessions and delegated routing. Churn — the rate at which nodes join and leave — degrades routing table quality; Kademlia mitigates this with least-recently-seen eviction, but mobile clients still perform poorly. Sybil attacks are inherent: nothing prevents an adversary from creating IDs close to a target key to poison lookups (the S/Kademlia paper proposes crypto-puzzle mitigations). Privacy leakage is fundamental — peer IPs are effectively public, which is why torrent-monitoring companies scrape the Mainline DHT directly. Bandwidth cost from maintenance traffic is nontrivial on metered connections.

### STUN / TURN / ICE

These three RFCs (STUN: 5389/8489, TURN: 5766/8656, ICE: 8445) address NAT traversal rather than discovery per se, but a discovery layer that terminates at "I have your peer's identity, now connect" requires them.

STUN is a single UDP round-trip: the client sends a `Binding Request` to a STUN server (`stun.l.google.com:19302` is the default in most WebRTC stacks), which echoes back the observed source IP:port. This reveals the client's external mapping and, combined with multiple probes, the NAT's behavior (endpoint-independent, address-dependent, symmetric). Symmetric NATs generally cannot be hole-punched and force TURN fallback.

TURN is a relay: the client allocates a relay address on a TURN server and instructs peers to send traffic there. Bandwidth is expensive — every byte flows through the relay — and providers like Twilio, Xirsys, and Cloudflare charge accordingly. Self-hosted `coturn` is common for those who own the traffic profile.

ICE is the orchestration protocol. Each peer gathers candidates (host: local interfaces; srflx: STUN-observed; relay: TURN-allocated), ranks them by priority, exchanges them via signaling, and runs connectivity checks (STUN Binding Requests over the candidate pairs) until one succeeds. Trickle ICE (RFC 8838) lets candidates flow as they're discovered rather than batching.

Empirical hole-punching success rates from Tailscale and others suggest 70-85% of NAT pairs succeed without TURN; the residual demands a relay.

### Signaling servers

WebRTC deliberately leaves signaling out of scope. Peers must exchange SDP offer/answer and ICE candidates via some out-of-band channel; the signaling server does this and nothing more. It sees connection metadata but not media.

- **Matrix rooms** function as signaling for Element Call, using room events to relay SDP.
- **Iroh's relay servers** (formerly "DERP" from Tailscale, which Iroh forked) double as both HTTPS-based signaling AND TURN-like relays when direct paths fail.
- **Jitsi Meet, Google Meet, Discord** all run proprietary signaling; simple demos use `socket.io` over a small Node.js server.

The distinction from a "central identity server" is important. Signaling doesn't authenticate users or persist identity; it's a rendezvous point for two parties who already know each other's identifiers. This is why federated signaling (Matrix) works: any homeserver can broker a connection.

### Tor hidden services

Tor onion services v3 derive `.onion` addresses from Ed25519 public keys (56 base32 characters). The service publishes descriptors to the Tor directory containing introduction points; clients build a circuit to an introduction point, negotiate a rendezvous point, and both parties establish a six-hop circuit (three hops each side) at the rendezvous.

The property that matters for local-first design is metadata resistance: neither peer learns the other's IP, and no external observer can correlate them without breaking Tor. Latency cost is the price — 500ms to several seconds for connection setup, hundreds of ms for round-trips. Bandwidth through Tor is also constrained (typically <10 MB/s per circuit).

Adoption in P2P messaging is meaningful. **Briar** uses Tor hidden services for all communication when internet-connected, falling back to Bluetooth and WiFi Direct locally. **Cwtch** is built on Ricochet's protocol, itself entirely Tor-based. **OnionShare** exposes file transfer and chat over ephemeral onion services. For any application where reachability of a NAT'd device matters more than latency, hidden services are a reasonable primary transport.

### Bluetooth LE and local radio

BLE advertising uses the 2.4 GHz ISM band with three primary advertising channels (37, 38, 39). Peripherals broadcast advertising packets (up to 31 bytes of payload in legacy advertising, 255 bytes with LE 5.0 extended advertising) that centrals scan for. GATT (Generic Attribute Profile) then structures services and characteristics for connected communication. iBeacon and Eddystone are conventions layered on advertising payloads for identification without connection.

AirDrop combines BLE for peer detection with AWDL for bulk transfer; the BLE payload contains a hashed contact identifier that Apple devices match against the local address book. Google's Nearby Share does something similar. FindMy and AirTag piggyback on any nearby iPhone to relay location.

Range is typically 10-30m indoors, less through walls. Pairing UX remains a persistent friction point despite BLE improvements. iOS restricts background BLE scanning aggressively — apps in background can only scan for pre-registered service UUIDs, and even then with reduced duty cycle. Android's behavior varies by OEM. Bluetooth MAC randomization (mandatory on iOS, optional but common on Android) breaks naive tracking but also complicates legitimate re-identification across sessions.

---

## Part 2 — Decentralized Identity Systems

For buzzy, "identity" has three jobs: give Alice a durable name that survives across her devices, let peers verify a message truly came from Alice, and do both without a central account server.

### Ed25519 keypair-as-identity

The simplest model: *your public key is your name*. Ed25519 (RFC 8032) is the modern default — a 32-byte public key, a 64-byte signature, deterministic (no nonce reuse footguns like ECDSA), safe curve parameters (twist-secure, no invalid-curve attacks), and fast enough that verification is essentially free.

- **Signal** uses a long-term Ed25519 identity key per user, plus X3DH prekey bundles: a signed prekey and a batch of one-time prekeys. New sessions derive Curve25519 DH shares from these; Ed25519 signs the bundle so recipients can verify authenticity of the DH material. The identity key is what "safety numbers" hash to.
- **Keybase** issues a per-device Ed25519 signing key and chains device additions/revocations into a *sigchain* — an append-only log signed by prior devices. The user identity is the root, but each device speaks for itself. This is the closest existing model to what buzzy needs.
- **SSH** with `~/.ssh/id_ed25519` treats the pubkey as the identity. `authorized_keys` on the server is an allowlist; `known_hosts` is TOFU for host keys. No CA, no directory — the key is the credential.
- **age** (`age-encryption.org`) uses X25519 recipient keys formatted as `age1...` bech32 strings. There is no identity concept beyond "can this key decrypt" — pure cryptographic capability.

Compared to older primitives: RSA-2048 keys are 256 bytes, slower, and historically riddled with padding bugs; ECDSA on P-256 is fine but requires safe randomness for every signature. Ed25519's determinism eliminates that class of failure, and its short pubkey fits in a QR code or a URL fragment.

### DIDs (W3C Decentralized Identifiers)

DIDs are the W3C's attempt to standardize "identifier that resolves to a document containing keys and service endpoints." Format is `did:method:identifier` — the method is the resolution protocol. In 2026 the notable methods are:

- **`did:key`** — the identifier *is* the pubkey, base58-encoded with a multicodec prefix. No resolution required; the DID document is generated from the key. Effectively a rebranded Ed25519 pubkey with a JSON wrapper.
- **`did:web`** — resolves via HTTPS at `https://example.com/.well-known/did.json`. Depends on DNS and TLS; not really decentralized, but pragmatic.
- **`did:plc`** — AT Protocol / Bluesky's method. A signed operation log stored on a "PLC directory" server, so key rotation is possible without changing the DID. Bluesky's directory is a single service today; the plan to federate it has moved slowly.
- **`did:ion`** — Microsoft's Sidetree-on-Bitcoin method. Technically impressive, operationally fragile; adoption has largely stalled.

The value DIDs add over a raw pubkey is *key rotation and service discovery*: the DID document can list current keys, previous keys, and endpoints (e.g., a messaging inbox). The cost is real: DID documents, resolvers, verification relationships (`authentication`, `assertionMethod`, `keyAgreement`), and a JSON-LD context that most implementations don't fully process. Adoption outside AT Protocol and a handful of SSI wallets (verifiable credentials for education/finance pilots) is thin.

For buzzy, DIDs are overkill unless the wire format needs to interoperate with the SSI ecosystem. A raw `did:key` gives nothing a base58 pubkey doesn't; a `did:plc`-style operation log gives rotation but requires either a directory or a gossip protocol to distribute updates. Keybase-style sigchains achieve the same rotation property without adopting the W3C stack.

### Petnames and Zooko's triangle

Zooko's triangle: identifiers can be at most two of *secure*, *decentralized*, *human-meaningful*. Pubkeys are secure and decentralized but not memorable. DNS names are secure and memorable but centralized. Nicknames are memorable and decentralized but not secure (anyone can claim "Alice").

Nick Szabo's petnames paper (2005) resolves the trilemma by layering: the global identifier is the secure/decentralized one (a pubkey), and each user assigns a *local* human-meaningful label. My "Bob" and your "Bob" needn't be the same person; my client resolves "Bob" to a pubkey I've saved.

Implementations:
- **SDSI/SPKI** (Rivest & Lampson, 1996) — linked local namespaces predating petnames by a decade.
- **Agoric's petname system** — object-capability petnames in Endo/Endo.
- **Farcaster and AT Protocol** — handles (`@alice.bsky.social`) are DNS-verified petnames pointing at DIDs. Global-looking but structurally local: the handle can change; the DID doesn't.
- **Syncthing** — Device ID is a truncated hash of the device's certificate (secure, decentralized); Device Name is a local label the user picks per contact. Two Syncthing users can label the same device differently with no conflict.

Syncthing's model maps directly onto buzzy's needs: peer identity is a keypair fingerprint; display name is a locally-scoped, user-editable string. Add QR-code introduction (each device shows its fingerprint, the other scans) and you have a workable UX without a name service.

### OIDC/OAuth binding to cryptographic identity

An orthogonal approach: bootstrap trust from an existing account (Google, GitHub, corporate SSO) by having the OIDC provider vouch for a pubkey.

- **Bluesky's OAuth+DID model** binds OAuth sessions to the user's `did:plc`; clients prove control of the DID during the flow.
- **Fediverse actor keys** — every ActivityPub actor publishes an RSA/Ed25519 pubkey in its actor JSON; HTTP Signatures on federated requests are verified against it.
- **SSH keys signed via OIDC** — Smallstep, Teleport, HashiCorp Boundary, and GitHub's SSH-with-OIDC issue short-lived certificates. The user authenticates to an OIDC provider; a CA signs their SSH pubkey with a short TTL; the target host trusts the CA. Sigstore does the same for artifact signing.

For buzzy, OIDC-bound identity is useful in one narrow case: a user wants to introduce a new device without physical access to an existing one. Signing a challenge with the OIDC-linked key lets a new device join the "Alice" set. This adds a trust dependency (the OIDC provider) but removes the "I'm travelling and my only device died" failure mode. Make it opt-in.

### Web of trust vs TOFU

PGP's web of trust asked users to sign each other's keys at keysigning parties. It largely failed for non-technical users: the mental model of "signing a key" is opaque, revocation was fragile, and the signature graph rarely had useful paths. Its lesson is not that WoT is wrong but that it can't be the primary UX.

TOFU (Trust On First Use) — accept the key you see the first time, warn loudly on change — is what actually ships:

- **SSH `known_hosts`** — canonical TOFU. Works because host keys rarely change and the warning on mismatch is unmissable.
- **Signal safety numbers** — a hash of both parties' identity keys. Users can compare once (in person, over a call), and Signal warns on rekey. Most users never verify; the value is that a MITM would eventually be visible.
- **Matrix cross-signing** — each user has a master signing key; devices are signed by the master; other users verify the master once via emoji SAS. Achieves multi-device identity with a single verification event per contact.

Real-world MITM against a first-use TOFU exchange is rare when the discovery channel is out-of-band (QR code across the room, a link sent through a different channel). It becomes plausible when introductions go through a hostile server. For buzzy, TOFU with a Signal-style "safety number changed" alarm is enough for the developer audience; a Matrix-style master-key model is what non-technical users need for the multi-device case.

### Identity comparison

| Approach | Verifiable w/o central server | UX cost | Crypto strength | Ecosystem maturity |
|---|---|---|---|---|
| Ed25519 pubkey-as-identity | Yes | Low (fingerprint/QR) | Excellent | Very high (SSH, Signal, age) |
| DID (`did:key`) | Yes | Medium (JSON overhead) | Same as underlying key | Niche (SSI, AT Proto) |
| DID (`did:plc`, `did:web`) | Partial (directory dependency) | Medium-high | Depends on directory | AT Proto: growing; else thin |
| Petnames layer | Yes | Very low (local labels) | Inherits from key | High (Syncthing, Farcaster) |
| OIDC-bound keys | No (needs OIDC provider) | Low (familiar) | High (short-lived certs) | High for infra, low for consumer P2P |
| PGP WoT | Yes | Very high | High (if keys valid) | Effectively dead outside Debian |
| TOFU + rekey warnings | Yes | Low | Depends on first-contact channel | High (SSH, Signal, Matrix) |

---

## Part 3 — Six Tools Compared: Identity, Discovery, NAT, Trust, Multi-Device

### Syncthing

- **Identity**: SHA-256 fingerprint of self-signed TLS cert, base32 with Luhn check digits every 13 chars, printed as five dash-separated groups (`LYXKCHX-VI3NYZR-...`, 56 chars). Identity == cert; rotating cert = new identity.
- **Discovery**: LAN via UDP broadcast on 21027 + IPv6 multicast to `[ff12::8384]:21027`; WAN via global discovery servers (`discovery.syncthing.net`, anycast, stateless `DeviceID → [address, ...]` HTTPS API); explicit static addresses per device.
- **NAT**: TCP → QUIC (with opportunistic hole punching) → relay pool (`relays.syncthing.net`, community-donated Go relays, tunneled block-exchange protocol). No STUN/TURN/ICE. Two symmetric-NAT peers essentially always use a relay.
- **Trust**: pairwise TOFU + out-of-band Device ID exchange. **Introducer flag** delegates trust: if device A trusts device B as an introducer, A auto-accepts new devices B vouches for on shared folders. Convenient, also the biggest footgun — a compromised introducer silently expands your trust set.
- **Multi-device**: none native. Two of your laptops = two Device IDs. You share folders between them the same way you share with a friend. There is no "user account." N² pairing problem introducers only partially paper over.
- **Weaknesses**: no forward secrecy at identity layer (long-lived cert keys); no key rotation story (rotate = new identity = re-pair everywhere); discovery servers see every announcement (who is online, from what IP); introducer transitivity is coarse (all-or-nothing per introducer); relay pool depends on volunteer capacity; no push/wake for offline devices. The block-level rsync-style sync itself is excellent; the identity and rendezvous plumbing shows its 2013 origins.
- **Copy**: self-signed key as identity, LAN mDNS/broadcast alongside a stateless WAN rendezvous. **Avoid**: identity==cert coupling; introducer as the primary multi-user abstraction.

### Signal

- **Identity**: long-term Ed25519 IdentityKey per device + signed prekey + one-time prekeys; PQXDH (2023) added Kyber1024 last-resort + one-time PQ prekeys. User handle: phone number (E.164) transitioning to usernames + per-conversation ACI (Account Identity, a UUID). Safety numbers = deterministic hash of both parties' identity keys, 60 digits displayed as 5×12.
- **Discovery**: none in a P2P sense — federated at operator level (one operator). Contact Discovery runs inside Intel SGX enclaves against a hashed-and-encrypted directory. All data plane traffic goes through Signal's servers.
- **NAT**: N/A for messages — the server is always the middleman. Voice/video uses their own SFU (`Signal-Calling-Service`) with ICE/DTLS-SRTP.
- **Trust**: TOFU on identity keys, with safety number surfaced for OOB verification. Visible "safety number changed" warning on rekey. No central directory of who-is-trusted; each device makes its own TOFU decisions.
- **Multi-device**: linked devices, QR-scan provisioning. Each linked device has its own IdentityKey registered under the user's ACI. Group messages fan out per-device (sender keys optimize this). PQXDH (2023) added post-quantum forward secrecy to X3DH. Storage service (encrypted, server-hosted) syncs contacts and settings.
- **Weaknesses**: phone number as identity remains the dominant flow — hard to onboard the phone-averse, hard to have a persona separate from a SIM. Linked-device model is one-primary: lose the phone and desktops eventually deauthorize. Contact discovery, even in enclaves, requires trust in Intel SGX and Signal's operational integrity. Everything routes through one operator — great for metadata hiding via sealed sender, terrible for censorship resistance.
- **Copy**: safety-number-style deterministic pairwise fingerprints for OOB verification; the discipline of surfacing key-change events. **Avoid**: phone number as identity; single-operator server; a "primary device" that all others depend on.

### Matrix (with Element)

- **Identity**: two layers. **Server-level identity** is `@localpart:homeserver.tld` — a Matrix ID (MXID) bound to a homeserver account. **Cryptographic identity** is per-device Curve25519 + Ed25519 keys (Olm for 1:1 and Megolm for group ratchets) plus **cross-signing** (MSC1756): a Master Signing Key (MSK), a Self-Signing Key (SSK) that signs your own devices, and a User-Signing Key (USK) that signs *other users' MSKs* you've verified. The MSK is what a "user" cryptographically is.
- **Discovery**: federated. Your homeserver resolves `@bob:matrix.org` via `.well-known/matrix/server` and the S2S API; homeservers gossip room state to each other. No LAN discovery, no P2P mode in production (a P2P Matrix demo has existed since 2020 using libp2p, but it is not shipped in Element).
- **NAT**: N/A for messages (server-relayed). Voice/video: full-mesh WebRTC (small calls) or LiveKit/Element Call SFU (larger calls) with TURN. Homeservers can advertise a TURN server via `/voip/turnServer`.
- **Trust**: cross-signing plus device verification via emoji SAS (MSC1267 — 7 emoji chosen from a 64-emoji set, compared OOB). Verifying another user means your USK signs their MSK, which propagates trust to all their signed devices automatically. **Genuinely well-designed: verify once per human, not once per device pair.**
- **Multi-device**: first-class. New device generates its own device key, gets signed by your SSK (after you approve from an existing verified device, or via Secure Secret Storage & Backup — SSSS — where your cross-signing private keys are encrypted with a recovery passphrase and stored on the homeserver). Message history sync uses server-side encrypted key backup (MSC1219). Losing all devices is survivable if you kept the recovery key.
- **Weaknesses**: complexity is the biggest cost. Cross-signing, SSSS, key backup, verification, room keys, backup keys, and their failure modes generate the majority of Element support traffic. "Unable to decrypt" (UTD) errors have haunted the ecosystem for years and are largely artifacts of the key-distribution mesh across federated servers. Homeservers see the full social graph and message metadata. Federation is theoretically decentralized, practically 80% on `matrix.org`. No offline LAN mode.
- **Copy**: cross-signing hierarchy (a per-user master key that signs per-device keys) so verification amortizes across devices; SAS-style OOB verification with a small, memorable alphabet. **Avoid**: the SSSS-plus-backup-key-plus-recovery-passphrase UX; treat key custody as one problem, not three.

### Iroh (n0)

- **Identity**: NodeID = raw Ed25519 public key, 32 bytes, encoded as base32 without padding (52 chars). See `iroh_base::NodeId`. There is no certificate, no wrapping structure — the pubkey *is* the name. QUIC connections are authenticated by proving possession of the corresponding secret key.
- **Discovery**: Iroh separates **discovery** (map NodeID → addresses) from **relaying** (fallback data path). Discovery mechanisms are pluggable: mDNS/`swarm-discovery` on LAN, DNS-based discovery via `dns.iroh.link` (a NodeID resolves to a `_iroh_node.<hash>.dns.iroh.link` TXT record containing home relay + direct addresses), and pkarr (public-key-addressable resource records) over Mainline DHT. DNS-based discovery is the default WAN path.
- **NAT**: relay servers speak a protocol derived from Tailscale's DERP. Nodes maintain a persistent WebSocket-ish connection to their "home relay," which serves both as a rendezvous (peers can send packets to a NodeID via the relay) and as a fallback data path. When two peers connect, they exchange candidate addresses through the relay, then attempt QUIC hole punching (using STUN-like probes on the relay side). If direct fails, all traffic continues through the relay, encrypted end-to-end.
- **Trust**: pure pubkey authentication with no PKI. Whoever holds the private key is that NodeID. Applications built on Iroh are expected to layer their own trust semantics on top (iroh-docs, iroh-blobs, iroh-gossip do this differently).
- **Multi-device**: no opinion at the protocol layer — each node is its own identity. The application above must handle "these three NodeIDs are all Alice." iroh-docs (their eventually-consistent document CRDT) has authors, which are separate keypairs from NodeIDs; an author can sign from multiple nodes. That's the closest thing to a user abstraction, and it's deliberately application-level.
- **Weaknesses**: deliberate minimalism at the identity layer means every application reinvents users, groups, and revocation. Relay dependence is real: without a reachable relay, symmetric-NAT peers can't find each other at all, and the default relays are operated by n0 the company. DHT discovery via pkarr is promising but ecosystem-thin. Documentation is engineer-facing; there is no non-technical onboarding story yet.
- **Copy**: raw-pubkey-as-NodeID simplicity; the DERP-style always-on relay-plus-hole-punch combo; **the separation between "how do I find you" (discovery) and "how do I send you bytes" (transport) — the cleanest architectural split of any tool in the list**. **Avoid**: assuming the application will happily invent its own user model — that hand-off is where non-technical UX dies.

### Tailscale (and WireGuard)

- **Identity**: each node has a WireGuard Curve25519 keypair (the "node key"), a separate Ed25519 machine key, and a NLPub for Tailnet Lock (Ed25519, opt-in). The user identity is external: Google, Microsoft, GitHub, Okta, or self-hosted OIDC via Headscale. The coordination server binds `user@idp → [node keys]`. MagicDNS maps `hostname.tailnet.ts.net` to the node's Tailscale IP (a CGNAT `100.64.0.0/10` address).
- **Discovery**: fully centralized coordination. Each node long-polls the coordination server (`/machine/map`) and receives a `NetMap` describing every other reachable node's pubkey, endpoints (STUN-discovered public ip:port candidates), routes, and DERP home region. Zero P2P discovery on LAN in the traditional sense — but LAN peers do exchange direct endpoints via the coordination server and then connect directly over WireGuard. Headscale is protocol-compatible.
- **NAT**: DERP relays (`derp*.tailscale.com`) form a global mesh; each node picks the lowest-latency one as "home." Endpoint discovery uses embedded STUN in DERP servers. Hole punching is standard birthday-paradox-style simultaneous send over UDP, coordinated by DERP forwarding an initial packet ("disco" protocol). Falls back to DERP relay if direct fails. **Reference implementation the field mostly copies** — the Crawshaw/Fitzpatrick essay "How NAT traversal works" is the plain-English standard.
- **Trust**: SSO-delegated. The IdP asserts "this is alice@example.com," coord server issues a signed netmap, WireGuard keys are bound to that assertion. Tailnet Lock (optional) adds an offline-signable trust anchor so a compromised coord server can't silently add nodes. ACLs are policy files, not per-peer TOFU.
- **Multi-device**: excellent. One user, N nodes, all under the same tailnet, all reachable by name via MagicDNS. Adding a device is `tailscale up` and clicking a browser SSO flow. Arguably the most polished multi-device UX in the list.
- **Weaknesses**: the coordination server is a single trust root (and a business asset — vendor lock without Headscale). SSO dependence means identity is only as decentralized as your IdP. WireGuard itself has no key rotation on the wire (Tailscale rotates every 180 days by re-issuing via coord). It's a VPN, not a sync tool — the model is "everyone is on the same LAN," which is powerful but wrong for pure P2P sync semantics where you want opportunistic mesh, not persistent tunnels.
- **Copy**: DERP-style relays; the disco/hole-punching protocol (battle-tested, well-documented); MagicDNS-style human-readable names built from cryptographic identity; the ergonomics of `tailscale up`. **Avoid**: mandatory coordination server; SSO as the only identity onramp; VPN-shaped mental model.

### IPFS / libp2p

- **Identity**: PeerId = multihash of the peer's public key. Historically SHA-256 of the protobuf-serialized pubkey, base58-encoded with a `Qm...` prefix (46 chars, "CIDv0"). Modern form is CIDv1 with base32 lowercase, prefix `bafz...`. For Ed25519 keys ≤ 42 bytes, the "identity" multihash inlines the key itself — the PeerId literally *is* the key, no hashing.
- **Discovery**: Kademlia DHT (the "Amino" public DHT for the main IPFS network) is the primary WAN mechanism — providers announce `PeerId → [multiaddr, ...]` records, resolvers query by XOR-nearest routing. Bootstrap nodes (hardcoded list) seed initial DHT connectivity. LAN discovery via mDNS (`_ipfs-discovery._udp.local`). Rendezvous protocol (`/libp2p/rendezvous/1.0.0`) for topic-based discovery. Pubsub (GossipSub) for broadcast.
- **NAT**: AutoNAT (`/libp2p/autonat/1.0.0`) determines reachability by asking other peers to dial you back. If unreachable, DCUtR (Direct Connection Upgrade through Relay, `/libp2p/dcutr`) coordinates hole punching through Circuit Relay v2. Circuit Relay v2 has time and byte limits (unlike v1's unlimited relay) to prevent free-rider abuse. Reservation-based: you must ask a relay to hold a slot for you.
- **Trust**: pubkey-authenticated at the transport layer (TLS 1.3 with a libp2p extension, or Noise); no built-in trust anchor beyond "this is the PeerId that dialed me." Applications layer trust on top. IPNS provides a mutable pointer signed by a PeerId's private key, published via the DHT or PubSub. There is no PKI, no cross-signing, no user concept.
- **Multi-device**: none at the libp2p layer. IPNS with a shared key across devices is the usual hack, but that shares the private key, which is the opposite of proper multi-device. Ceramic, OrbitDB, and other higher layers each invent their own users. This is a well-known gap in the stack.
- **Weaknesses**: DHT lookups are slow (multi-second p99), leaky (queries are observable), and only reliable when your node is publicly reachable. Bootstrap node dependency is real. Circuit relay v2 is the only supported relay flavor but reservation logic is complex and the ecosystem's relay capacity is uneven. Multiaddrs (`/ip4/1.2.3.4/tcp/4001/p2p/QmFoo`) are expressive but user-hostile. The AutoNAT-then-DCUtR dance has more moving parts than DERP's approach and correspondingly more failure modes.
- **Copy**: the *idea* of multiaddrs (transport-agnostic addresses composed of typed segments — `/ip4/.../udp/.../quic/p2p/...`) is genuinely useful; PeerId-as-inline-key for small keys is a nice optimization. **Avoid**: the DHT as the primary WAN discovery path (too slow, too observable, too dependent on public reachability); pushing every trust and user concern up to the application without opinion.

### Cross-tool patterns

Reading across the six, patterns for a LAN-first sync tool that must eventually support non-technical users:

- **Identity = raw Ed25519 pubkey**, not a certificate (Iroh, libp2p have it right; Syncthing's cert coupling is a burden).
- **User = master key that signs device keys**, per Matrix's cross-signing. One human, N devices, one verification act per peer human. Biggest gap in Iroh and libp2p, biggest strength of Matrix and Tailscale.
- **Discovery is pluggable, transport is separate.** Iroh's split is the cleanest.
- **LAN: mDNS.** Ubiquitous, well-understood, works with zero config. UDP-broadcast fallback (Syncthing) for networks that block mDNS.
- **WAN: DERP-style relay + hole punch**, not DHT. Tailscale/Iroh have shown this works reliably; libp2p's DHT-first approach has not.
- **Trust: SAS-style OOB verification (Matrix, Signal) as the paved path.** Introducers (Syncthing) are a footgun; SSO (Tailscale) is a great optional onramp but a bad only choice.
- **Metadata honesty: assume the rendezvous sees who's online.** Syncthing's global discovery is honest about this; hide the sync content (E2E) but don't pretend the relay is blind.
- **Onboarding: `tailscale up` is the bar.** One command, one QR code, device is on the mesh under the user's identity. Anything more complex loses non-technical users.

The design that isn't in any single existing tool: raw-pubkey NodeIDs, a Matrix-style cross-signing user layer above them, a Tailscale/Iroh DERP-plus-hole-punch transport, mDNS on LAN, and a stateless pubkey-lookup service on WAN that a paranoid user can swap for their own without changing anything else.

---

## Part 4 — NAT Traversal Techniques

### NAT taxonomy and hole punching

Classical RFC 3489 taxonomy (full-cone / restricted / port-restricted / symmetric) was replaced by **RFC 4787** (BCP 127, 2007), which decomposes NAT behavior into two orthogonal axes: **mapping** (endpoint-independent / address-dependent / address-and-port-dependent) and **filtering** (same three). REQ-1 mandates endpoint-independent mapping for new NATs; REQ-2 says the external IP must remain stable ("IP address pooling paired"). Old-taxonomy "symmetric NAT" ≈ address-and-port-dependent mapping.

Naive hole punching fails on symmetric NAT because the mapping the STUN server observes is bound to the STUN server's `IP:port`. When the peer probes a different destination, the NAT assigns a fresh external port that STUN never reported. RFC 5128 §3.4 notes that port prediction "has little chance of working if either client is behind two or more levels of NAT."

**Ford, Srisuresh & Kegel (USENIX ATC 2005)** remains the most cited empirical baseline: 380 UDP data points across 68 vendors gave **UDP hole-punching success 82% (310/380); TCP 64% (184/286); UDP hairpin 24%; TCP hairpin 13%**. Belkin, Cisco, SMC, and 3Com scored 100% on UDP; Linksys 98%; Netgear 84%; Draytek 12%. RFC 5128 rounds this to ">80% UDP, >60% TCP, up to 88% with advanced techniques."

CGNAT changes the picture. RFC 6888 (2013) recommends but does not require endpoint-independent filtering. Corporate networks compound this by disabling UPnP and often filtering non-443 outbound UDP entirely. The "80-90% direct P2P on residential" number holds for consumer broadband but drops sharply behind mobile CGNAT and hotel/enterprise/university networks.

Tailscale's "How NAT traversal works" documents a **birthday-paradox port-prediction** technique: with 256 open ports on the hard side, 174 probes give 50% collision probability, 1024 probes 98%. Two hard NATs squares the search space to ~4×10⁹, requiring **~54,000 probes for 50% (~9 min) and ~170,000 for 99.9% (~28 min)**. Some consumer routers (Juniper SRX 300, capped 64,000 sessions) exhaust their session table before the probe completes.

### STUN / TURN / ICE — current RFCs

- **STUN**: RFC 8489 (Feb 2020, obsoleting 5389). Lightweight request/response over UDP/TCP/TLS/DTLS on default port 3478 (5349 for TLS). Only method: Binding. XOR-MAPPED-ADDRESS (§14.2) returns the client's observed `IP:port` XOR'd with the magic cookie `0x2112A442` so on-path NATs cannot rewrite it.
- **TURN**: RFC 8656 (Feb 2020, obsoleting 5766). The client `Allocate`s an ephemeral relayed transport address (default lifetime 600s, max 3600s) and communicates through it via Send/Data indications (36-byte overhead per packet) or ChannelBind (4-byte header, 10-minute lifetime). URIs use `turn:` and `turns:` schemes.
- **ICE**: RFC 8445 (Jul 2018). Each endpoint gathers candidates of four types: host, server-reflexive (STUN), peer-reflexive (discovered mid-check — typical of symmetric NAT), and relayed (TURN). Priority is `2²⁴·type_pref + 2⁸·local_pref + (256 − component_id)` with recommended type preferences 126 / 100 / 110 / 0. Connectivity checks are STUN Binding requests on the exact 5-tuples that will carry media; the controlling agent nominates a working pair using USE-CANDIDATE.
- **Trickle ICE**: RFC 8838 (Jan 2021) exchanges candidates incrementally as discovered. SDP transport lives in RFC 8840; JSEP (RFC 8829) glues it to WebRTC.

**TURN pricing (2025)**: Twilio Network Traversal Service $0.40/GB in US-West and Frankfurt, $0.60-$0.80/GB in Asia and South America; STUN is free. **Cloudflare Realtime TURN is $0.05/GB standalone**, roughly 8× cheaper. In WebRTC deployments, historical rule of thumb: Twilio cited ~8% of calls fall back to TURN (2015), WebRTCHacks ~20%. Post-CGNAT numbers likely trend higher.

### QUIC connection migration

QUIC (RFC 9000) is not a hole-punching protocol, but its transport model tolerates the path changes hole punching produces. Connections are keyed by Connection ID rather than the 5-tuple (§5.1); NEW_CONNECTION_ID and RETIRE_CONNECTION_ID frames rotate CIDs so an on-path observer cannot link flows across a rebinding. When a client migrates (WiFi ↔ cellular, or a NAT re-mapping), it uses PATH_CHALLENGE / PATH_RESPONSE frames (§8.2) to prove reachability and return routability on the new path before shifting the session over. Only clients migrate in v1.

The `preferred_address` transport parameter (§18.2, §9.6) lets a server advertise an alternate `IPv4/IPv6:port` plus CID and stateless-reset token during the handshake, so post-handshake the client can migrate off an anycast or load-balancer address to a stable node.

**`draft-seemann-quic-nat-traversal`** (Seemann and Kinnear, Apple; expired individual submission, not WG-adopted) proposes reusing PATH_CHALLENGE/PATH_RESPONSE as ICE-style candidate probes on the QUIC socket. Long-header demux lets STUN share the port; on a successful probe, connection migration moves the session onto the direct path without QUIC renegotiation. Iroh and libp2p already run QUIC over hole-punched UDP as a matter of implementation; the draft would make it interoperable.

### Iroh and Tailscale DERP

**Tailscale's DERP** ("Designated Encrypted Routing Protocol for Packets") is designed as an always-on relay of last resort and a NAT-traversal side channel. It runs over HTTP/HTTPS on ports 80/443 (with STUN on 3478), which passes through networks that block arbitrary outbound UDP. Two packet types cross it: DISCO discovery frames and encrypted WireGuard payloads. **Private keys never leave the client** — a DERP server "blindly forwards already-encrypted traffic." Every Tailscale client picks a home DERP by latency, starts each connection on DERP, and transparently upgrades to a direct path once hole punching succeeds. Path selection has been RTT-based since v0.100.0 rather than the ICE default of preferring LAN over WAN. Roughly 26 metros across six continents, with ≥3 servers per region.

**Iroh's `iroh-relay` crate** inherits this design: HTTPS upgrade to a raw TCP relay protocol, forwarding encrypted traffic keyed by `EndpointId` (a public key). Relays additionally provide QUIC Address Discovery, HTTP endpoints, and ICMP echo for hole-punching coordination. Iroh's `magicsock` (name inherited from Tailscale's Go implementation) transparently multiplexes direct and relayed paths.

Structural contrast with TURN: TURN is a UDP relay with per-client server-allocated `IP:port` and mandatory authentication; DERP is an HTTPS relay with public-key addressing and (in the public deployment) no authentication. Tailscale explicitly rejected TURN because "there's no real interoperability benefit since there are no open TURN servers on the internet."

Both Tailscale (blog: "a direct connection over 90% of the time") and Iroh (docs: "roughly 9 out of 10 networking conditions allow a direct connection") converge on **~10% of sessions needing sustained relay**. Neither publishes a formal breakdown by NAT type.

### WebRTC ICE in browsers

Browsers gather all four candidate types. Since **Chrome 76 (August 2019)**, host candidates are obfuscated as `<uuid-v4>.local` mDNS names — 122 bits of entropy, scoped to the page origin — resolved by the peer via multicast DNS. The mechanism lives in `draft-ietf-mmusic-mdns-ice-candidates-03`, which expired without becoming an RFC; the draft measured the impact as "the observed impact to ICE connection rate was 2% (relative) when mDNS was enabled on both sides," with STUN-required connections rising from 94% to 97%. RFC 8828 defines the policy modes for IP exposure (`no_host`, `default_public_interface_only`, `default_public_and_private_interfaces`, `all_interfaces`) but not the mDNS mechanism itself.

The public STUN server most sites use is `stun:stun.l.google.com:19302` (plus `:3478` and `stun1..stun4`), anycasted with no SLA. Signalling is out of scope: applications carry SDP offer/answer (RFC 3264, JSEP RFC 8829) over any channel they like — WebSocket, XMPP/Jingle (XEP-0166), Matrix, or a custom REST endpoint. Trickle ICE (RFC 8840 for SDP, XEP-0176 for XMPP) means candidates flow as discovered rather than in a single bundle.

### UPnP IGD / NAT-PMP / PCP

Three protocols let a device self-provision an inbound port on its gateway.

- **UPnP IGD** (2001 / v2 2010) uses SOAP over HTTP, discovered via SSDP on UDP 1900. Actions include `AddPortMapping`, `DeletePortMapping`, `GetExternalIPAddress`; IGD2 adds `AddAnyPortMapping` and IPv6 firewall control. **Classic UPnP has no authentication** — any host on the LAN can open holes — which is one reason enterprises disable it wholesale.
- **NAT-PMP** (RFC 6886, Apple, Informational) is a compact binary UDP protocol on port 5351, shipped in Mac OS X 10.4 (2005) and AirPort base stations.
- **PCP** (RFC 6887, Standards Track, 2013) extends NAT-PMP with third-party mapping, outbound flow anchoring (PEER opcode), and rapid-recovery announcements. PCP is what RFC 6888 recommends for CGNAT subscriber-facing mapping control (REQ-9).

Security has been rough. **CallStranger (CVE-2020-12695, CVSS 7.5 High)** allowed subscription callbacks to arbitrary URLs, enabling data exfiltration, DDoS reflection, and internal port scanning. Historical miniupnpd bugs include CVE-2013-0230 (stack overflow, RCE), CVE-2019-12107 (uninitialised memory in miniupnpc), and CVE-2017-1000494 (NULL deref via malformed SOAP). BitTorrent clients (µTorrent, qBittorrent, Transmission, Deluge, libtorrent-based) call `AddPortMapping` on startup for both TCP and UDP listen ports. Corporate CIS Benchmarks and NIST SP 800-41 guidance push disablement, so a design that depends on UPnP will fail behind most enterprise gateways.

### Modern IETF work

- **libp2p DCUtR** — Direct Connection Upgrade through Relay, r1 2021-11-20, protocol ID `/libp2p/dcutr`. Two peers already sharing a Circuit Relay v2 connection open a `/libp2p/dcutr` stream on it. Inbound peer B sends `Connect` (msg type 100) with its observed addrs and starts an RTT timer; A replies in kind; B sends `Sync` (msg type 300) and waits **RTT/2** to align a simultaneous open. TCP mode produces TCP Simultaneous Open; QUIC mode has A dial on Sync while B sprays random-length UDP packets at 10-200 ms intervals to punch the NAT. Up to 3 retry attempts; on failure the relay connection persists. Wire format is length-prefixed protobuf, 4 KiB max.
- **WebRTC in libp2p** (r1 2023-04-12) defines two variants: WebRTC (private ↔ private via libp2p signalling) and WebRTC Direct (browser ↔ public server using self-signed DTLS fingerprints embedded in the multiaddr, avoiding CA-issued certs). It uses Circuit Relay v2 + DCUtR instead of TURN. Implementations in js-libp2p and rust-libp2p; go-libp2p in progress.
- **MASQUE / RFC 9298** (Aug 2022, Standards Track) proxies UDP inside HTTP using Extended CONNECT with `:protocol=connect-udp`. Payload is HTTP Datagrams (RFC 9297) via the Capsule Protocol on HTTP/2 and QUIC DATAGRAM frames (RFC 9221) on HTTP/3. A SOCKS-like escape hatch through networks that pass only HTTPS.

---

## Part 5 — Key Management UX for Non-Technical Users

### Signal: making key management invisible

Signal's design principle is that cryptography should be a background property, not a user-facing feature. On install, the client generates an Ed25519 long-term identity key and a Curve25519 signed prekey plus one-time prekeys, uploading the public halves to Signal's server. The user sees only a phone-number prompt. The phone number acts as a human-readable handle mapping to a key bundle; the mapping is opaque to the user.

Several things are deliberately hidden. Private Contact Discovery, running inside Intel SGX enclaves since 2017 (later expanded with attestation via AWS Nitro), lets the client learn which contacts have Signal without the server learning the address book. Sealed Sender strips the sender identifier from the envelope so the server sees only the recipient. Key rotation happens on every message via the Double Ratchet (X3DH for the initial handshake, then chained HKDF derivations per message); users are unaware. The Sesame protocol handles multi-device fanout under the hood.

Exposed to users: the 60-digit safety number (also displayed as a QR code), a "safety number changed" banner when the peer's identity key changes, and a "verified" toggle. That is the entire user-visible key management surface.

**Signal's PQXDH transition** (announced September 2023, at `signal.org/docs/specifications/pqxdh/`) added a CRYSTALS-Kyber-1024 KEM alongside the existing X25519 exchange, producing a hybrid shared secret. From the user's perspective it was invisible: no new UI, no safety-number churn, no re-verification. Possible because PQXDH augments the initial handshake without changing the long-term Ed25519 identity key from which the safety number derives. **The strongest existence proof that protocol upgrades can be fully invisible when identity keys are held stable across the transition.**

### Safety numbers and verification compliance

Every major E2EE messenger derives a fingerprint from both parties' identity public keys and asks users to compare it out-of-band:

- **Signal safety numbers**: SHA-512 truncated to 60 digits, displayed in five-digit groups plus a QR code.
- **WhatsApp "security code"**: same protocol, 12 groups of five digits or a QR.
- **Matrix emoji SAS** (MSC1267): after an ECDH handshake, both sides derive 40 bits of entropy and map them to 7 emoji from a fixed 64-emoji dictionary.
- **Wire**: hex fingerprint per device.
- **Threema**: QR codes with three trust levels colour-coded red/orange/green.
- **Briar**: QR exchange over Bluetooth or shared local network required before any messaging is possible.

Empirical picture from usable-security research is bleak:
- **Vaziripour et al. (SOUPS 2017, "Is that you, Alice?")**: even after being explicitly told to verify their partner in a lab study, only 14% of Signal users could complete safety-number comparison without guidance; when guided, completion rates rose but times remained long (minutes).
- **Schröder et al. (2016, "When Signal Hits the Fan")**: users misinterpreted the "safety number changed" warning, often clicking through without understanding it implied a possible MITM.
- **Herzberg and Leibowitz** follow-ups: similar patterns across WhatsApp and Signal.

**Consistent finding**: users don't verify, and when forced they prefer QR scans over reading digit strings by a wide margin. Emoji SAS lands between — faster than digits, less reliable than in-person QR because emoji rendering varies across platforms and some pairs (grinning-face vs smiling-face) confuse users.

Implication for buzzy: assume the base rate of manual verification is near zero. Any security property depending on the user reading a string is effectively opt-in for the paranoid few. QR-in-person during first pairing is the one moment users will tolerate a verification step.

### Key backup and recovery

E2EE is fundamentally at odds with "I lost my phone." If the provider cannot decrypt, the provider cannot restore. Each system chooses a compromise:

| Approach | How it works | Threat model | Failure mode |
|---|---|---|---|
| **Signal SVR** | v1 SGX; v2 (2023) threshold across 3 SGX enclaves; v3 (2024) + AWS Nitro attestation. Argon2 from 4+ digit PIN → Shamir 2-of-3 → attested enclaves with 10-attempt hard limit | Trusts enclave attestation, tolerates low-entropy PIN | Enclave compromise / side-channel |
| **iCloud Keychain + Advanced Data Protection** (iOS 16.2+) | Per-device keys sync via iCloud Keychain, encrypted with a key derived from the device passcode, protected by Apple's HSM cluster. ADP (opt-in) removes Apple's recovery ability; user must maintain recovery contact or 28-char recovery key | Trusts secure enclave + user keeps recovery key | User loses recovery key. iCloud Backup separate from Keychain; holds message content Apple-decryptable unless ADP on |
| **WhatsApp encrypted backups** (October 2021+, opt-in) | 64-digit key or password. With password, key is derived and stored in a Backup Key Vault (HSM-based). White paper at `engineering.fb.com/2021/09/10/security/whatsapp-e2ee-backups/` | HSM attestation | Low adoption because backups worked before this |
| **Matrix SSSS + Key Backup** (MSC1946 Secret Storage) | Passphrase-derived key or 58-char recovery key encrypts per-user secret store. Message-key backup separately encrypted with Curve25519 keypair whose private half lives in SSSS | User keeps passphrase/recovery key | UX notoriously confusing; users routinely lose recovery key; Element has iterated the flow multiple times |
| **BIP39 mnemonics** (Bitcoin, Ethereum, Monero) | 12 or 24 words from 2048-word list, encoding 128 or 256 bits + checksum | Trusts only the user | Loss is permanent — works for high-value assets where users self-select for care; doesn't scale to normal messaging |
| **Shamir social recovery** (Vault12, Argent, ERC-4337 modules) | Split a recovery key across trusted guardians; reconstruction requires k-of-n approvals | Threshold of guardians | Guardians unreachable, collude, or lose their shares |

For buzzy, a PIN-plus-attested-enclave design is the pragmatic default; BIP39 as an advanced option honors the developer-first audience.

### Multi-device identity — two structural patterns

**Pattern A: one identity key, per-device subkeys signed by it.**
Matrix cross-signing (MSC1756) is canonical: each user has a Master Signing Key (MSK) held offline or in SSSS; it signs a Self-Signing Key (SSK) that in turn signs each device's Ed25519 key, and a User-Signing Key (USK) that signs other users' MSKs after verification. Verifying one user's MSK transitively trusts all their devices, and verifying between users is done once per pair rather than n×m per device pair. iMessage under Advanced Data Protection is a similar shape: the Apple ID owns a hardware-backed identity, devices are child keys.

**Pattern B: peer device keys with a signed roster.**
Keybase's per-device sigchain is the reference: each device has its own key, and joining the account requires an existing device to countersign a new entry into the user's append-only sigchain. Any device can revoke another. Signal linked devices are a pragmatic hybrid: the phone is the primary; iPad, desktop, and Linux clients get their own Curve25519 keys but the phone must scan a QR to authorize them, and Sesame keeps session state consistent. Peergos uses a similar per-device model with owner signatures.

Trade-offs:
- **Pattern A** concentrates trust in one identity key: verify once, trust everywhere. Elegant, but the identity key is catastrophic to lose or leak; hence offline / SSSS storage requirement.
- **Pattern B** distributes trust across devices. Each device is a peer; losing one doesn't lose the identity. But adding a new device requires an already-authorized device online, and every peer needs to see the roster update, which is fragile in intermittently connected sync systems.

**For a decentralized sync system like buzzy, Pattern B fits better than Pattern A.** No phone-as-primary assumption to make; devices are peers by nature. A signed device roster stored in the buzzy data plane and replicated like any other CRDT can serve as the transparency log. Onboarding follows the Signal QR-pairing gesture: an existing device signs the new device's key into the roster, the user watches a QR scroll by, everyone else picks up the roster change on next sync.

### UX patterns worth stealing

- **QR-code pairing.** The single unambiguously successful verification UX. Point camera, done. Works because it moves the entropy from the user's short-term memory to the physics of the room. Signal, WhatsApp, Matrix, Threema, Briar all converge on this for device linking; adopt it for buzzy device onboarding.
- **BIP39-style recovery phrases** for developers and power users. Twelve or twenty-four words is a well-understood UX shape; wallet users have taught themselves to write them down. Offer alongside a PIN-plus-enclave fallback.
- **Automatic key transparency logs.** Keybase pioneered per-user append-only sigchains anchored in Bitcoin; Google published a KT design for Messages in 2023; WhatsApp launched Key Transparency in April 2023 (auditable append-only log of public identity keys, verified by client on each conversation). Invisible to users but let the client automatically detect if the server ever hands out a different key for a peer. The correct model for buzzy: keys published to a verifiable log, client audits silently, warns only on mismatch.
- **"Verify your other device" prompts.** Element and Signal now nudge users to verify a newly linked device from an existing one before it can decrypt history. Frames verification as a *device pairing* task, not as an abstract cryptographic exercise, which lands better with users.
- **Invisible protocol upgrades** (Signal PQXDH). Design the identity layer so cipher-suite migrations don't disturb the identity key or its derived safety number. Users should never learn what a KEM is.

**Failure modes to internalize.** Safety-number verification will not happen; do not gate any critical property on it. Recovery keys will be lost; provide at least two independent recovery paths (PIN-plus-enclave and a mnemonic phrase, for instance). Cross-device state divergence is the single most common bug in multi-device E2EE; test it explicitly. Every meaningful E2EE system has had a "safety number changed" false-positive that trained users to ignore the warning; write the copy to distinguish "peer re-installed" (common, benign) from "possible MITM" (rare, serious), and offer a one-tap re-verification via QR.

**The through-line**: users will not do cryptography. They will scan a QR once, they will write down a phrase if a wallet has taught them to, they will type a PIN. Everything else must be invisible.

---

## Consolidated Recommendation for buzzy

### Identity layer

- **Device identity** = raw Ed25519 pubkey, 32 bytes, base32 no padding, printed with Luhn check chars every N (Syncthing's ergonomic lead). Not a certificate — Syncthing's identity==cert coupling is a rotation burden.
- **User identity** = master signing key ("UserID") that signs a **device roster** — an append-only, replicated (as a CRDT-signed log) list of device keys the user has authorized. Follows Keybase sigchain / Matrix cross-signing precedent. Any authorized device can sign in another; revocation is a signed roster entry.
- **Pattern B** (peer devices + signed roster) over Pattern A (offline master + subkeys) — no phone-as-primary in a P2P sync system; devices are peers.
- **Display** = petnames. Global fingerprint + user-editable local label per contact. Never a global name service.
- **Trust** = TOFU + rekey warnings. Optional OOB safety-number comparison via QR for security-conscious users. Master-key change requires explicit user acceptance.
- **No DIDs** unless SSI interop becomes a hard requirement. `did:key` adds nothing over a raw pubkey; `did:plc`-style rotation is achieved by the sigchain.

### Discovery layer (separated from transport, per Iroh)

- **LAN**: mDNS/DNS-SD (`_buzzy._udp.local` or similar). UDP-broadcast fallback (Syncthing pattern) for networks that block multicast.
- **WAN**: small fleet (3-5 regions) of stateless pubkey → address rendezvous servers. Same servers double as DERP-style relays. **Prefer this over a DHT** for the developer-first phase — cold-lookup latency (2-10s) and IP-visibility make Kademlia a poor default. DHT can be an opt-in privacy/censorship-resistance mode later.
- **Optional Tor hidden-service transport** as an opt-in adversarial-network mode (Briar precedent).

### Transport / NAT stack

Attempt order:
1. **Cached direct path** (last-known-working IP:port with ~500ms timeout; QUIC connection migration handles rebinding).
2. **LAN mDNS peer**, direct QUIC.
3. **STUN-observed candidates** on both sides + coordinated hole-punch via relay, trickle-style.
4. **DCUtR-style RTT/2-aligned QUIC hole punch** through the relay for both-behind-NAT.
5. **Relay fallback** (DERP-style): traffic flows through the relay while background probes keep trying to upgrade. Tailscale's model — start on relay, upgrade transparently.
6. **MASQUE / HTTPS-443 escape hatch** for hostile networks (hotels, DPI firewalls).

**Skip TURN entirely** — DERP-style relays cover the same territory with pubkey addressing, no auth setup, HTTPS transport, and much cheaper self-hosting than the TURN market ($0.40-$0.80/GB Twilio vs Cloudflare Realtime $0.05/GB vs your own relay for pennies of bandwidth).

**Transport = QUIC over UDP.** Connection migration handles NAT rebinding; PATH_CHALLENGE/RESPONSE handles path validation; QUIC-native hole-punching (Seemann draft-style, or the libp2p WebRTC-Direct approach) is well-tested in Iroh and libp2p.

**Treat UPnP / PCP / NAT-PMP as opportunistic optimization only** — never load-bearing. The CVE history (CallStranger CVSS 7.5, miniupnpd RCEs) and enterprise disablement make this a soft-fail path.

**Expected relay rate: ~10-20% sustained** (Tailscale + Iroh converge on ~10%; add margin for CGNAT-heavy mobile). Budget bandwidth accordingly; DERP-style forwarding of already-encrypted traffic keeps operational cost bounded.

Edge cases that will always need relay:
- Mobile CGNAT (T-Mobile, Jio) — address-and-port-dependent mapping + short port lifetimes.
- Enterprise firewalls blocking outbound UDP except whitelisted destinations.
- Symmetric NAT on both sides (birthday-paradox works but at 9+ min and 50k+ packets — bad developer UX).
- Hotel / café / airport captive networks.
- Hairpin failures (13-24% per Ford 2005).
- Corporate networks with UPnP disabled and PCP unavailable.

### Key management UX

- **Device onboarding**: existing authorized device shows a QR containing an ephemeral pairing token + relay address. New device scans, does an authenticated key exchange, existing device signs new device key into the roster. Non-technical users learn one gesture (scan QR). Follows Signal / Tailscale precedent.
- **Recovery** (for total device loss): two independent paths so users can pick.
  - **PIN + attested-enclave backup** (Signal SVR precedent, `buzzy-recovery` service): PIN → Argon2 → Shamir-split master key across attested enclaves with a 10-attempt hard limit. Default path for non-technical users.
  - **BIP39 recovery phrase**: 24 words at setup, "write this down." Developer / power-user mode. Optional; PIN path exists so users who lose the phrase aren't dead.
  - Optional **OIDC binding** as a third recovery path — sign a challenge with the OIDC-linked pubkey to authorize a new device onto the roster. Depends on the OIDC provider; opt-in.
- **Key transparency**: publish the device roster and user master key to a verifiable append-only log (Keybase / WhatsApp KT / Google KT precedent). Clients audit silently; warn only on mismatch. No user-visible surface unless something is wrong.
- **Verification UX**: safety-number comparison exists but is opt-in and only surfaced when the user opens the "Verify" flow. Copy for master-key change distinguishes "peer added a new device" (common) from "peer's master key changed" (rare, potentially serious) with a clear one-tap re-verify via QR.
- **Invisible protocol upgrades**: design the identity/wire layers so cipher-suite changes don't disturb the master key or the fingerprint the user compared. PQXDH-style hybrid handshake changes should be invisible.

### Onboarding bar

`bzz up` (or the daemon equivalent already in your design) + QR-scan pairing = device on the mesh under the user's identity. Tailscale sets this bar; anything more complex loses non-technical users. The developer audience gets a CLI-only path (`bzz device add --key <fingerprint>`) that skips the QR step but produces the same signed roster entry.

### Bill of Materials the buzzy protocol should adopt

- **Curve**: Ed25519 signing, X25519 KEM (Curve25519 DH), option to hybridize with Kyber-1024 later per PQXDH precedent.
- **Wire transport**: QUIC (RFC 9000) with connection migration + PATH_CHALLENGE.
- **Fingerprint format**: base32 no padding + Luhn checks; short display form (first N chars) for casual reference; full form for verification.
- **Rendezvous**: HTTPS-on-443 pubkey lookup service, same servers as DERP-style relays.
- **Signaling**: on same relay connection, per libp2p DCUtR + Tailscale disco precedent.
- **LAN discovery**: mDNS + UDP-broadcast fallback.
- **User layer**: Master Signing Key + append-only signed device-roster CRDT log, replicated with the rest of the buzzy data plane.
- **Trust**: TOFU on first contact, master-key change → explicit accept; SAS-style QR safety-number available on demand.
- **Backup**: PIN + attested enclave (default), BIP39 24-word phrase (opt-in), OIDC binding (opt-in).
- **Key transparency**: append-only public-key log, silent client audit.

The design that isn't in any single existing tool: raw-pubkey NodeIDs with a Matrix-style user layer above them (Pattern B roster), Iroh/Tailscale DERP + hole-punch transport, mDNS on LAN, stateless pubkey lookup on WAN swappable for self-hosted, QR-pairing + BIP39 + optional OIDC for recovery, invisible-by-default key management with opt-in verification for the paranoid.
