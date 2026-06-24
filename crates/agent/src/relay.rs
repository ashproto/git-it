//! Phase 2b-S5 transport: the LIVE end-to-end-encrypted relay. Convex carries only
//! ciphertext — every command/reply rides inside the HPKE+Ed25519 envelope
//! (`crypto::message`), routed by the recovery-key-signed device roster
//! (`crypto::roster`). The relay row's `kind` is always `"e2e"`; the real command
//! kind is sealed inside. This is fail-closed: a non-`e2e` row, an unverifiable
//! roster, an un-enrolled/revoked sender, a bad signature, or a replay are all
//! refused — there is no plaintext path.
//!
//! Flow: subscribe to this device's inbox AND the account roster; keep a current
//! `VerifiedRoster` (TOFU-pinning the account recovery pubkey on first sight); for
//! each inbound `e2e` message run `process_inbound` (decode → roster liveness →
//! verify-sig-first → anti-replay-after-auth), dispatch on the sealed kind, then
//! seal the reply back to the sender. Until the agent is enrolled into the roster
//! by the phone, commands stay pending (un-acked) and are retried once it is.

use std::{collections::{BTreeMap, HashSet}, env};
use convex::{ConvexClient, FunctionResult, Value};
use futures::StreamExt;
use serde_json::json;

use crate::crypto::enroll::{self, ChainRow};
use crate::crypto::keys::DeviceKeys;
use crate::crypto::message::{self, now_ms, random_nonce};
use crate::crypto::replay::ReplayGuard;
use crate::crypto::roster::VerifiedRoster;

const B64_STD: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub async fn run() -> anyhow::Result<()> {
    dotenvy::from_path(crate::repos::env_path()).ok();
    let url = env::var("CONVEX_URL").map_err(|_| anyhow::anyhow!("set CONVEX_URL to the deployment URL"))?;
    // This agent's stable identity = the device id captured at pairing (falling
    // back to a fresh hardware-UUID read), matched against the session token's
    // `did` claim server-side and used for message routing.
    let me = crate::repos::load_device_id().unwrap_or_else(crate::repos::device_id);
    // Authenticate: load the pairing credentials and present a session JWT the
    // fetcher re-mints on connect + every reconnect. Refuse to start unpaired.
    let auth = crate::auth::AgentAuth::load()
        .ok_or_else(|| anyhow::anyhow!("not paired — run `git-it-agent pair <code>` first"))?;
    // The agent's long-lived device keys (generated at pairing). Without them it
    // cannot decrypt anything — refuse to start rather than run a useless relay.
    let keys = crate::crypto::keys::load()
        .ok_or_else(|| anyhow::anyhow!("missing device keys — run `git-it-agent pair <code>` to enroll"))?;

    let mut client = ConvexClient::new(&url).await?;
    client.set_auth_callback(Some(auth.make_fetcher())).await;
    let mut writer = client.clone();
    let mut querier = client.clone();

    let mut inbox_args = BTreeMap::new();
    inbox_args.insert("to".into(), Value::String(me.clone()));
    let mut inbox_sub = client.subscribe("messages:inbox", inbox_args).await?;
    let mut roster_sub = client.subscribe("roster:chain", BTreeMap::new()).await?;

    // The current verified roster (None until the phone has published the recovery
    // key + enrolled us). The persisted anti-replay set spans the whole session.
    let mut roster: Option<VerifiedRoster> = None;
    let mut replay = ReplayGuard::load(crate::repos::replay_path(), now_ms());
    let mut seen: HashSet<String> = HashSet::new();
    let mut last_inbox: Vec<Value> = Vec::new();

    // presence heartbeat: upsert this device every 30s (fires immediately).
    let mut hb = client.clone();
    let hb_me = me.clone();
    tokio::spawn(async move {
        loop {
            let mut a = BTreeMap::new();
            a.insert("deviceId".into(), Value::String(hb_me.clone()));
            a.insert("name".into(), Value::String(crate::repos::device_name()));
            a.insert("kind".into(), Value::String("mac".into()));
            a.insert("armed".into(), Value::Boolean(crate::repos::armed()));
            if let Err(e) = hb.mutation("devices:heartbeat", a).await { eprintln!("heartbeat error: {e}"); }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    // Observe credential revocation (a 401 from /auth/refresh after unpair) so a
    // revoked agent exits cleanly instead of reconnect-spinning on a dead token.
    let revoked = auth.revoked_flag();
    println!("agent relay (e2e): authenticated as {me}; awaiting roster…");
    loop {
        tokio::select! {
            maybe = inbox_sub.next() => {
                let rows = match maybe {
                    Some(FunctionResult::Value(Value::Array(rows))) => rows,
                    Some(_) => continue,   // ignore non-array results
                    None => break,         // subscription ended
                };
                last_inbox = rows.clone();
                process_inbox(&rows, &me, &keys, roster.as_ref(), &mut replay, &mut seen, &mut writer).await?;
            }
            maybe = roster_sub.next() => {
                let chain = match maybe {
                    Some(FunctionResult::Value(v)) => v,
                    Some(_) => continue,
                    None => break,
                };
                // On a None result (empty / unverifiable / unpersistable roster) we
                // intentionally KEEP the current roster: a startup with no roster
                // stays closed, and a previously-verified roster is retained so the
                // untrusted relay can't disable us by injecting one bad chain.
                if let Some(v) = refresh_roster(&mut querier, &chain).await {
                    let became_ready = roster.is_none();
                    roster = Some(v);
                    // Now that we may be enrolled, re-pump any messages that arrived
                    // (and were left pending) before the roster verified.
                    if became_ready && !last_inbox.is_empty() {
                        process_inbox(&last_inbox, &me, &keys, roster.as_ref(), &mut replay, &mut seen, &mut writer).await?;
                    }
                }
            }
            _ = async {
                while !revoked.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            } => {
                anyhow::bail!("agent credential revoked (unpaired) — re-pair with `git-it-agent pair`");
            }
        }
    }
    Ok(())
}

/// Verify the freshly-fetched roster chain. TOFU-pins the account recovery public
/// key (fetched from Convex `account:recoveryPub`) on first sight, then resolves
/// the chain into the live device set and advances the persisted epoch floor.
/// Returns `None` (fail closed — serve no commands) on an empty/unpublished roster,
/// a missing recovery key, a pin conflict, or any verification failure.
async fn refresh_roster(querier: &mut ConvexClient, chain: &Value) -> Option<VerifiedRoster> {
    let rows = parse_chain_rows(chain);
    if rows.is_empty() {
        return None; // not enrolled yet — stay closed
    }
    // The recovery key is the root of trust. Pin it once (known-hosts model); a
    // later different key is refused by `pin_recovery_pub`, keeping us closed.
    let recovery_pub = match enroll::pinned_recovery_pub() {
        Some(p) => p,
        None => {
            let fetched = fetch_recovery_pub(querier).await?;
            if let Err(e) = enroll::pin_recovery_pub(&fetched) {
                eprintln!("roster: refusing recovery key — {e}");
                return None;
            }
            fetched
        }
    };
    match enroll::resolve_roster(&rows, &recovery_pub, enroll::roster_epoch_floor()) {
        Ok(v) => {
            // Fail closed if the new floor can't be DURABLY persisted: adopting a
            // roster whose floor isn't on disk would let a relay replay this epoch
            // range after a restart (the persisted floor would still be the old,
            // lower value). Keep the previously-verified roster (whose floor WAS
            // persisted) instead — its floor still blocks any real rollback.
            if let Err(e) = enroll::advance_epoch_floor(v.head_epoch) {
                eprintln!("roster: refusing new roster — could not persist epoch floor: {e}");
                return None;
            }
            println!("roster verified at epoch {} ({} live device(s))", v.head_epoch, v.live.len());
            Some(v)
        }
        Err(e) => {
            // A failed verification (garbage / forged / rolled-back chain from the
            // untrusted relay) is NOT adopted. The caller keeps the last VERIFIED
            // roster (whose epoch floor blocks any real rollback), so a relay cannot
            // disable the agent by injecting one bad chain — it can only WITHHOLD
            // updates, which is unavoidable and bounded by the floor.
            eprintln!("roster verification rejected (keeping last verified roster): {e}");
            None
        }
    }
}

/// Handle a batch of inbox rows. Each row is an `e2e` envelope; decrypt + authenticate
/// it, dispatch the sealed command, and seal the reply back to the sender. A message
/// that can't yet be processed because the roster isn't ready is left PENDING (not
/// acked, not marked seen) so it is retried once enrollment lands; every other
/// outcome (handled, or a hard auth/format failure) is acked + remembered so the
/// untrusted relay can't wedge the inbox by re-delivering it.
async fn process_inbox(
    rows: &[Value],
    me: &str,
    keys: &DeviceKeys,
    roster: Option<&VerifiedRoster>,
    replay: &mut ReplayGuard,
    seen: &mut HashSet<String>,
    writer: &mut ConvexClient,
) -> anyhow::Result<()> {
    for row in rows {
        let Value::Object(m) = row else { continue };
        let msg_id = str_field(m, "msgId");
        let kind = str_field(m, "kind");
        let body = str_field(m, "body");
        if msg_id.is_empty() || seen.contains(&msg_id) { continue; }

        // Flag-day fail-closed: the relay category MUST be "e2e". A plaintext or
        // unknown kind is refused (acked away so it can't re-deliver forever).
        if kind != "e2e" {
            eprintln!("refused non-e2e message {msg_id} (kind={kind})");
            ack(writer, &msg_id).await?;
            seen.insert(msg_id);
            continue;
        }

        // Until the roster verifies we can neither authenticate the sender nor seal
        // a reply — leave the message pending and retry after enrollment.
        let Some(roster) = roster else { continue };

        let now = now_ms();
        match message::process_inbound(me, &keys.x25519_priv, roster, replay, &body, &msg_id, now) {
            Ok((opened, sender_x25519)) => {
                let sender_kind = roster.keys_for(&opened.from).map(|k| k.kind.as_str()).unwrap_or("");
                let cmd = String::from_utf8_lossy(&opened.body);
                // When disarmed, refuse every command (read + write) EXCEPT
                // `setArmed` — otherwise a disarmed agent could never be re-armed
                // from the phone (permanent lockout). The heartbeat keeps reporting
                // so the phone shows "online but locked".
                let (reply_kind, reply_body) = if !crate::repos::armed() && opened.kind != "setArmed" {
                    ("error".to_string(), json!({ "message": "agent is disarmed" }).to_string())
                } else {
                    handle(&opened.kind, &cmd)
                };
                // Seal the reply back to the sender's x25519 key (from the roster).
                let reply_id = format!("{msg_id}-r");
                match message::seal_message(
                    &sender_x25519, &keys.ed25519_seed, me, &opened.from, &reply_id,
                    roster.head_epoch, now, random_nonce(), &reply_kind, reply_body.as_bytes(),
                ) {
                    Ok(wire) => {
                        send_e2e(writer, &reply_id, me, &opened.from, &wire).await?;
                        ack(writer, &msg_id).await?;
                        seen.insert(msg_id.clone());
                        println!("handled {} {msg_id} from {} ({sender_kind})", opened.kind, opened.from);
                    }
                    Err(e) => {
                        // Sealing failed (should not happen for a valid sender key) —
                        // drop it so it can't loop; we cannot reply.
                        eprintln!("failed to seal reply to {msg_id}: {e}");
                        ack(writer, &msg_id).await?;
                        seen.insert(msg_id.clone());
                    }
                }
            }
            Err(e) => {
                // Hard authentication/format failure (unknown/revoked sender against a
                // ready roster, bad signature, replay, msgId mismatch, malformed
                // envelope). Drop it — the relay is untrusted and must not be able to
                // re-deliver a forged message indefinitely.
                eprintln!("rejected message {msg_id}: {e}");
                ack(writer, &msg_id).await?;
                seen.insert(msg_id);
            }
        }
    }
    Ok(())
}

/// Send a sealed reply as an `e2e` relay row.
async fn send_e2e(writer: &mut ConvexClient, msg_id: &str, from: &str, to: &str, wire: &str) -> anyhow::Result<()> {
    let mut send = BTreeMap::new();
    send.insert("msgId".into(), Value::String(msg_id.to_string()));
    send.insert("from".into(), Value::String(from.to_string()));
    send.insert("to".into(), Value::String(to.to_string()));
    send.insert("kind".into(), Value::String("e2e".into()));
    send.insert("body".into(), Value::String(wire.to_string()));
    writer.mutation("messages:send", send).await?;
    Ok(())
}

/// Ack the original so it leaves the pending inbox.
async fn ack(writer: &mut ConvexClient, msg_id: &str) -> anyhow::Result<()> {
    let mut a = BTreeMap::new();
    a.insert("msgId".into(), Value::String(msg_id.to_string()));
    writer.mutation("messages:ack", a).await?;
    Ok(())
}

/// Parse a Convex `roster:chain` result (array of objects) into `ChainRow`s.
/// Malformed rows are skipped; the chain verifier independently fails closed if
/// the result is short or inconsistent.
fn parse_chain_rows(v: &Value) -> Vec<ChainRow> {
    let Value::Array(rows) = v else { return Vec::new() };
    rows.iter()
        .filter_map(|r| {
            let Value::Object(o) = r else { return None };
            let entry_b64 = match o.get("entryB64") { Some(Value::String(s)) => s.clone(), _ => return None };
            let sig_b64 = match o.get("sigB64") { Some(Value::String(s)) => s.clone(), _ => return None };
            Some(ChainRow { entry_b64, sig_b64 })
        })
        .collect()
}

/// Fetch the account recovery PUBLIC key from Convex `account:recoveryPub`.
/// Returns `None` if unpublished, the result is malformed, or the query errors.
async fn fetch_recovery_pub(querier: &mut ConvexClient) -> Option<[u8; 32]> {
    let res = querier.query("account:recoveryPub", BTreeMap::new()).await.ok()?;
    let FunctionResult::Value(Value::Object(o)) = res else { return None };
    let Some(Value::String(s)) = o.get("recoveryPubB64") else { return None };
    use base64::Engine;
    B64_STD.decode(s).ok()?.as_slice().try_into().ok()
}

fn handle(kind: &str, body: &str) -> (String, String) {
    match kind {
        "repoStatus" => {
            let repo = serde_json::from_str::<serde_json::Value>(body)
                .ok().and_then(|v| v.get("repoPath").and_then(|p| p.as_str()).map(String::from));
            match repo {
                Some(path) => match crate::status_json(&path) {
                    Ok(status) => ("repoStatusResult".into(), status),
                    Err(e) => ("error".into(), json!({ "message": e }).to_string()),
                },
                None => ("error".into(), json!({ "message": "missing repoPath" }).to_string()),
            }
        }
        "listRepos" => match serde_json::to_string(&crate::repos::list_repos()) {
            Ok(json) => ("reposResult".into(), json),
            Err(e) => ("error".into(), json!({ "message": e.to_string() }).to_string()),
        },
        "addRepo" => match repo_path(body) {
            Some(p) => { crate::repos::add_repo(&p); ("reposResult".into(), repos_result()) }
            None => ("error".into(), json!({ "message": "missing path" }).to_string()),
        },
        "removeRepo" => match repo_path(body) {
            Some(p) => { crate::repos::remove_repo(&p); ("reposResult".into(), repos_result()) }
            None => ("error".into(), json!({ "message": "missing path" }).to_string()),
        },
        "setArmed" => {
            // Require an explicit boolean. Never default this security flag to
            // the more-permissive (armed) state on a missing/garbage body — a
            // stray or replayed message must NOT silently re-arm a disarmed
            // agent. On parse failure, refuse and leave the current state.
            match serde_json::from_str::<serde_json::Value>(body).ok()
                .and_then(|v| v.get("armed").and_then(|a| a.as_bool()))
            {
                Some(want) => match crate::repos::set_armed(want) {
                    // Reply with the ACTUAL on-disk state (re-read), so a failed
                    // write is never reported as success — matching how the repo
                    // commands self-report via list_repos().
                    Ok(()) => ("armedResult".into(),
                        json!({ "armed": crate::repos::armed() }).to_string()),
                    Err(e) => ("error".into(),
                        json!({ "message": format!("failed to persist armed: {e}") }).to_string()),
                },
                None => ("error".into(),
                    json!({ "message": "missing or non-boolean armed" }).to_string()),
            }
        }
        other => ("error".into(), json!({ "message": format!("unknown kind: {other}") }).to_string()),
    }
}

/// Extract the `"path"` string field from a command body, if present.
fn repo_path(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body).ok()
        .and_then(|v| v.get("path").and_then(|p| p.as_str()).map(String::from))
}

/// Serialize the fresh repo list as the `reposResult` body (matches `listRepos`).
fn repos_result() -> String {
    serde_json::to_string(&crate::repos::list_repos()).unwrap_or_else(|_| "{}".into())
}

fn str_field(m: &BTreeMap<String, Value>, k: &str) -> String {
    match m.get(k) { Some(Value::String(s)) => s.clone(), _ => String::new() }
}
