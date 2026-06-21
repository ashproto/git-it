//! Slice-2 transport: handle typed commands over Convex. Subscribe to this
//! device's inbox; for each unacked command, dispatch on `kind`, reply, ack.
//! Still pre-auth / pre-encryption (body is plaintext JSON).

use std::{collections::{BTreeMap, HashSet}, env};
use convex::{ConvexClient, FunctionResult, Value};
use futures::StreamExt;
use serde_json::json;

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
    let mut client = ConvexClient::new(&url).await?;
    client.set_auth_callback(Some(auth.make_fetcher())).await;
    let mut writer = client.clone();

    let mut inbox_args = BTreeMap::new();
    inbox_args.insert("to".into(), Value::String(me.clone()));
    let mut sub = client.subscribe("messages:inbox", inbox_args).await?;
    let mut seen: HashSet<String> = HashSet::new();

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
    println!("agent relay: authenticated as {me}; listening…");
    loop {
        let rows = tokio::select! {
            maybe = sub.next() => match maybe {
                Some(FunctionResult::Value(Value::Array(rows))) => rows,
                Some(_) => continue,   // ignore non-array results
                None => break,         // subscription ended
            },
            _ = async {
                while !revoked.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            } => {
                anyhow::bail!("agent credential revoked (unpaired) — re-pair with `git-it-agent pair`");
            }
        };
        for row in rows {
            let Value::Object(m) = row else { continue };
            let msg_id = str_field(&m, "msgId");
            let kind = str_field(&m, "kind");
            let from = str_field(&m, "from");
            let body = str_field(&m, "body");
            if msg_id.is_empty() || seen.contains(&msg_id) { continue; }
            seen.insert(msg_id.clone());

            // when disarmed, refuse every command (read + write) EXCEPT
            // `setArmed` — otherwise a disarmed agent could never be re-armed
            // from the phone (permanent lockout). The heartbeat keeps reporting
            // so the phone shows "online but locked".
            let (reply_kind, reply_body) = if !crate::repos::armed() && kind != "setArmed" {
                ("error".to_string(), json!({ "message": "agent is disarmed" }).to_string())
            } else {
                handle(&kind, &body)
            };
            // reply to sender
            let reply_id = format!("{msg_id}-r");
            let mut send = BTreeMap::new();
            send.insert("msgId".into(), Value::String(reply_id));
            send.insert("from".into(), Value::String(me.clone()));
            send.insert("to".into(), Value::String(from));
            send.insert("kind".into(), Value::String(reply_kind));
            send.insert("body".into(), Value::String(reply_body));
            writer.mutation("messages:send", send).await?;
            // ack the original so it leaves the pending inbox
            let mut ack = BTreeMap::new();
            ack.insert("msgId".into(), Value::String(msg_id.clone()));
            writer.mutation("messages:ack", ack).await?;
            println!("handled {kind} {msg_id}");
        }
    }
    Ok(())
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
