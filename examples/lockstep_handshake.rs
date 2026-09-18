//! Two machines. No coordinator, no agreement protocol, no messages between them.
//!
//! Each one is handed the same three players' inputs in a DIFFERENT arrival order, and both
//! derive the same 64-bit history hash anyway. Then one of them is corrupted on a single tick,
//! is caught by hash comparison alone, and is rolled back onto the authoritative history.
//!
//! Run: `cargo run -p forge-core-v3 --example lockstep_handshake`
//!
//! This is the runtime face of `forge_core_v3::lockstep` (L23: a green test is STATIC, a
//! launched binary printing real numbers is RUNTIME).

use forge_core_v3::lockstep::{LockstepBarrier, Verdict};

/// Player inputs per tick: (peer, input_word). Input words are bitfields — 0x1 north,
/// 0x2 south, 0x4 east, 0x8 west, 0x10 strike.
const SCRIPT: [[(u8, u32); 3]; 5] = [
    [(0, 0x01), (1, 0x04), (2, 0x00)],
    [(0, 0x01), (1, 0x04), (2, 0x10)],
    [(0, 0x09), (1, 0x00), (2, 0x10)],
    [(0, 0x08), (1, 0x02), (2, 0x14)],
    [(0, 0x00), (1, 0x02), (2, 0x04)],
];

fn rule(ch: char, n: usize) -> String {
    core::iter::repeat(ch).take(n).collect()
}

fn banner(title: &str) {
    println!("\n{}", rule('=', 74));
    println!("  {title}");
    println!("{}", rule('=', 74));
}

/// Feed one tick to a barrier in the given peer order, then release it.
fn play_tick(m: &mut LockstepBarrier, tick: u64, order: &[usize], script: &[(u8, u32); 3]) -> u64 {
    for &i in order {
        let (peer, input) = script[i];
        m.submit(tick, peer, input).expect("input inside window");
    }
    let bundle = m.try_advance().expect("every peer submitted, so the tick releases");
    bundle.chain_hash
}

fn main() {
    banner("LOCKSTEP HANDSHAKE — determinism without communication");

    let mut alice = LockstepBarrier::new(3);
    let mut bob = LockstepBarrier::new(3);

    println!("  peers: {}   window: 16 ticks   inputs fold in PEER order", alice.peers());
    println!("  both machines start at the same basis: 0x{:016x}", alice.chain_hash());
    println!("  ALICE receives inputs in order p0,p1,p2   (the 'host')");
    println!("  BOB   receives them SHUFFLED p2,p0,p1     (arrival order must not matter)\n");

    println!("  tick |  inputs (p0,p1,p2)  |    alice chain    |     bob chain     | agree");
    println!("  -----+---------------------+-------------------+-------------------+------");

    for (t, script) in SCRIPT.iter().enumerate() {
        let tick = t as u64;
        let a = play_tick(&mut alice, tick, &[0, 1, 2], script);
        let b = play_tick(&mut bob, tick, &[2, 0, 1], script);

        println!(
            "  {:>4} | 0x{:02x} 0x{:02x} 0x{:02x}      | {:016x}  | {:016x}  |  {}",
            tick,
            script[0].1,
            script[1].1,
            script[2].1,
            a,
            b,
            if a == b { "YES" } else { "NO" }
        );
    }

    match alice.verify_remote(bob.chain_hash()) {
        Verdict::Match => {
            println!("\n  VERDICT: Match — 5 ticks, 15 inputs, zero bytes exchanged between them.");
            println!("  Shuffled arrival order changed nothing: the fold is by peer index.");
        }
        Verdict::Desync { tick, local, remote } => {
            println!("\n  VERDICT: Desync at tick {tick} — {local:016x} vs {remote:016x}");
        }
    }

    // ---------------------------------------------------------------------------------
    banner("CORRUPTION — one wrong bit on one peer, on one tick");

    let agreed_tick = alice.tick();
    let agreed_hash = alice.chain_hash();
    println!("  last agreed point: tick {agreed_tick}, chain {agreed_hash:016x}");

    let truth = [(0u8, 0x01u32), (1, 0x08), (2, 0x02)];
    let mut corrupt = truth;
    corrupt[1].1 = 0x0c; // peer 1: 0x08 -> 0x0c, a single extra bit

    for &(peer, input) in truth.iter() {
        alice.submit(agreed_tick, peer, input).unwrap();
    }
    for &(peer, input) in corrupt.iter() {
        bob.submit(agreed_tick, peer, input).unwrap();
    }
    let a = alice.try_advance().unwrap();
    let b = bob.try_advance().unwrap();

    println!("  alice tick {}: p1 = 0x{:02x}  ->  {:016x}", a.tick, truth[1].1, a.chain_hash);
    println!("  bob   tick {}: p1 = 0x{:02x}  ->  {:016x}", b.tick, corrupt[1].1, b.chain_hash);

    let verdict = alice.verify_remote(bob.chain_hash());
    match verdict {
        Verdict::Match => println!("\n  VERDICT: Match — WRONG, the corruption went undetected."),
        Verdict::Desync { tick, local, remote } => {
            println!("\n  VERDICT: Desync caught at tick {tick}");
            println!("    local  {local:016x}");
            println!("    remote {remote:016x}");
            println!("  One bit in one peer's input word moved the whole history hash.");
            println!("  No state had to be compared — only 8 bytes.");
        }
    }

    // ---------------------------------------------------------------------------------
    banner("ROLLBACK — adopt the authoritative history and reconverge");

    println!("  bob rolls back onto alice's point: tick {}, chain {:016x}", alice.tick(), alice.chain_hash());
    bob.rollback_to(alice.tick(), alice.chain_hash());
    println!("  queued inputs dropped: banked against a rejected history, so themselves rejected.\n");

    let resume = [(0u8, 0x10u32), (1, 0x01), (2, 0x08)];
    let t = alice.tick();
    for &(peer, input) in resume.iter() {
        alice.submit(t, peer, input).unwrap();
        bob.submit(t, peer, input).unwrap();
    }
    let a = alice.try_advance().unwrap();
    let b = bob.try_advance().unwrap();

    println!("  tick {} replayed on both:", a.tick);
    println!("    alice {:016x}", a.chain_hash);
    println!("    bob   {:016x}", b.chain_hash);

    match alice.verify_remote(bob.chain_hash()) {
        Verdict::Match => {
            println!("\n  VERDICT: Match — reconverged. Both machines are on one history again.");
        }
        Verdict::Desync { tick, local, remote } => {
            println!("\n  VERDICT: Desync at tick {tick} — {local:016x} vs {remote:016x}");
            println!("  Rollback FAILED to reconverge.");
        }
    }

    banner("RECEIPT");
    println!("  alice: tick {:>2}  chain {:016x}", alice.tick(), alice.chain_hash());
    println!("  bob:   tick {:>2}  chain {:016x}", bob.tick(), bob.chain_hash());
    println!("  identical: {}", alice.chain_hash() == bob.chain_hash());
    println!("  source: crates/forge-core-v3/src/lockstep.rs (gen-4 of 4, see .forge/repo-map.tsv)");
    println!();
}
