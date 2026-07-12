//! Verify waterfalls descriptor-scan against a live endpoint — a confidence
//! check before wiring waterfalls into a wallet app.
//!
//! It runs `get_waterfalls_all` for a descriptor, lists every transaction the
//! server saw (per derivation index), and — for each sighting — fetches the tx
//! and prints its outputs (address + value) so an incoming payment's amount is
//! visible.
//!
//! Usage:
//! ```sh
//! ESPLORA_CLIENT_ID=... ESPLORA_CLIENT_SECRET=... \
//!   cargo run --example waterfalls_query -- "<public-descriptor#checksum>" [to_index]
//! ```
//! Env:
//! - `ESPLORA_BASE_URL`   default `https://enterprise.blockstream.info/testnet/api/`
//! - `ESPLORA_CLIENT_ID` / `ESPLORA_CLIENT_SECRET` → enterprise Bearer; absent → public.
//! - `WATERFALLS_DESCRIPTOR`, `WATERFALLS_TO_INDEX` — alternatives to the CLI args.

use std::collections::BTreeSet;
use std::env;

use esplora_rs::Client;

/// test1@test.com's 3-of-3 `wsh(sortedmulti)` descriptor (pkcs11 test app).
/// Public data (tpub xpubs only). Override with arg 1 or `WATERFALLS_DESCRIPTOR`.
/// External index-0 receive address:
/// `tb1qw4vhwepl0f408dwx37fj3pmcgrg4c29drde39rtr0e9swge583eqy9drrl`.
const DEFAULT_DESCRIPTOR: &str = "wsh(sortedmulti(3,[28645006/48'/1'/0'/2']tpubDEwqCvJxKwKWX9xvRe48uofWJn1Y89Jn8UeH1Efrjb1UEVjUDy3URYTiqWaVCW7WdvHrL8XrSihHEhTwv5H3VDJoakjuCHiAnr6xcF2Xm4s/<0;1>/*,[73c5da0a/48'/1'/0'/2']tpubDFH9dgzveyD8zTbPUFuLrGmCydNvxehyNdUXKJAQN8x4aZ4j6UZqGfnqFrD4NqyaTVGKbvEW54tsvPTK2UoSbCC1PJY8iCNiwTL3RWZEheQ/<0;1>/*,[b8688df1/48'/1'/0'/2']tpubDEfobrrtptRTbKf4gysDhoabneABDTAcdj3Vbn4XwPsLE2pmqpizSPRG6zHsbAMuiSgWmWPsYCLHTKTPpyrGJ5rAoTpKoQNZcxodiPf2tSJ/<0;1>/*))";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let descriptor = args
        .next()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            env::var("WATERFALLS_DESCRIPTOR")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_DESCRIPTOR.to_string());
    let to_index: u32 = args
        .next()
        .or_else(|| env::var("WATERFALLS_TO_INDEX").ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let base = env::var("ESPLORA_BASE_URL")
        .unwrap_or_else(|_| "https://enterprise.blockstream.info/testnet/api/".to_string());

    let enterprise =
        env::var("ESPLORA_CLIENT_ID").is_ok() && env::var("ESPLORA_CLIENT_SECRET").is_ok();
    let client = if enterprise {
        println!("client : enterprise (Bearer) @ {base}");
        Client::new(&base)?
    } else {
        println!("client : public @ {base}");
        Client::new_public(&base)?
    };
    println!("scan   : to_index={to_index}");
    println!("desc   : {descriptor}\n");

    let resp = client.get_waterfalls_all(descriptor, to_index).await?;
    println!("tip    : {}", resp.tip);

    // Collect every sighting (branch, index, txid, status).
    let mut seen: Vec<(String, usize, String, String)> = Vec::new();
    for (branch, per_index) in &resp.txs_seen {
        for (idx, sightings) in per_index.iter().enumerate() {
            for s in sightings {
                let status = if s.height > 0 {
                    let ts = s
                        .block_timestamp
                        .map(|t| format!(", ts {t}"))
                        .unwrap_or_default();
                    format!("confirmed @ {}{ts}", s.height)
                } else {
                    "unconfirmed".to_string()
                };
                seen.push((branch.clone(), idx, s.txid.clone(), status));
            }
        }
    }

    if seen.is_empty() {
        println!("\nno history yet for this descriptor — fund an address and re-run.");
        return Ok(());
    }

    println!("\n{} sighting(s):", seen.len());
    for (branch, idx, txid, status) in &seen {
        let tag = if branch.contains("/0/*") {
            "recv"
        } else if branch.contains("/1/*") {
            "change"
        } else {
            "?"
        };
        println!("  [{tag} idx {idx:>3}] {txid}  ({status})");
    }

    // Enrich with amounts: fetch each unique tx and show its outputs.
    let unique: BTreeSet<String> = seen.iter().map(|(_, _, txid, _)| txid.clone()).collect();
    println!(
        "\noutputs (address → value sat) for {} tx(s):",
        unique.len()
    );
    for txid in &unique {
        match client.get_tx(txid).await {
            Ok(tx) => {
                println!("  tx {txid}");
                for vout in &tx.vout {
                    let addr = vout
                        .scriptpubkey_address
                        .as_deref()
                        .unwrap_or("(non-address)");
                    println!("    {addr}  {} sat", vout.value);
                }
            }
            Err(e) => println!("  tx {txid} (could not fetch: {e})"),
        }
    }

    println!("\n✅ waterfalls surfaced this descriptor's history in one descriptor query.");
    Ok(())
}
