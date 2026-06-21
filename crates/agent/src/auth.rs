//! Agent authentication (Phase 2a). The agent never self-signs: at pairing it
//! receives an opaque refresh token from the issuer and thereafter exchanges it
//! for a short-lived session JWT via `POST {site}/auth/refresh`. The session
//! token is presented to Convex through `ConvexClient::set_auth_callback`, whose
//! fetcher re-mints on connect + every reconnect (rotating the refresh token).

use std::{future::Future, pin::Pin, sync::Arc};

use convex::{AuthenticationToken, AuthTokenFetcher};
use tokio::sync::Mutex;

/// Loaded pairing credentials + the issuer site origin.
pub struct AgentAuth {
    site: String,
    account: String,
    /// The current refresh token; rotated on each refresh.
    refresh: Arc<Mutex<String>>,
}

impl AgentAuth {
    /// Load persisted credentials. Returns None if the agent isn't paired yet
    /// (run `git-it-agent pair <code>` first) or CONVEX_SITE_URL is unset.
    pub fn load() -> Option<AgentAuth> {
        let site = std::env::var("CONVEX_SITE_URL").ok()?;
        let refresh = crate::repos::load_refresh_token()?;
        Some(AgentAuth {
            site: site.trim_end_matches('/').to_string(),
            account: crate::repos::load_account(),
            refresh: Arc::new(Mutex::new(refresh)),
        })
    }

    /// Build the Convex auth fetcher: on each invocation it refreshes (and thus
    /// rotates) the session token. The network call happens OUTSIDE the lock; the
    /// lock is only held to read the current refresh token and to write+persist
    /// the rotated one (no `.await` while holding it across the network).
    pub fn make_fetcher(&self) -> AuthTokenFetcher {
        let site = self.site.clone();
        let account = self.account.clone();
        let refresh = self.refresh.clone();
        Box::new(move |_force_refresh: bool| {
            let site = site.clone();
            let account = account.clone();
            let refresh = refresh.clone();
            Box::pin(async move {
                let current = { refresh.lock().await.clone() };
                let (token, rotated) = http_refresh(&site, &current).await?;
                {
                    let mut g = refresh.lock().await;
                    *g = rotated.clone();
                    if let Err(e) = crate::repos::save_auth(&rotated, &account) {
                        eprintln!("warning: failed to persist rotated refresh token: {e}");
                    }
                }
                Ok(AuthenticationToken::User(token))
            }) as Pin<Box<dyn Future<Output = anyhow::Result<AuthenticationToken>> + Send>>
        })
    }
}

/// Exchange a refresh token for a fresh session JWT (+ a rotated refresh token).
async fn http_refresh(site: &str, refresh_token: &str) -> anyhow::Result<(String, String)> {
    let resp = reqwest::Client::new()
        .post(format!("{site}/auth/refresh"))
        .json(&serde_json::json!({ "refreshToken": refresh_token }))
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!(
            "session refresh failed: HTTP {} — re-pair with `git-it-agent pair`",
            resp.status()
        );
    }
    let v: serde_json::Value = resp.json().await?;
    let token = v.get("token").and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("refresh response missing token"))?;
    let rotated = v.get("refreshToken").and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("refresh response missing refreshToken"))?;
    Ok((token.to_string(), rotated.to_string()))
}

/// `git-it-agent pair <code>`: claim a phone-issued pairing code. Provisions this
/// Mac (DISARMED) under the phone's account and persists the refresh token. Also
/// sets the LOCAL armed flag to false so the disarm gate matches the
/// provisioned-disarmed device row until the human arms it from the phone.
pub async fn run_pairing(code: &str) -> anyhow::Result<()> {
    dotenvy::from_filename(".env.local").ok();
    let site = std::env::var("CONVEX_SITE_URL")
        .map_err(|_| anyhow::anyhow!("set CONVEX_SITE_URL to the deployment's .convex.site origin"))?;
    let site = site.trim_end_matches('/');
    let device_id = crate::repos::device_id();
    let device_name = crate::repos::device_name();
    let resp = reqwest::Client::new()
        .post(format!("{site}/auth/agent/claim"))
        .json(&serde_json::json!({ "code": code, "deviceId": device_id, "deviceName": device_name }))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("pairing failed: HTTP {status} — {body}");
    }
    let v: serde_json::Value = resp.json().await?;
    let refresh = v.get("refreshToken").and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("claim response missing refreshToken"))?;
    let account = v.get("account").and_then(|x| x.as_str()).unwrap_or("");
    crate::repos::save_auth(refresh, account)?;
    // Fresh pair = disarmed locally, matching the disarmed device row.
    crate::repos::set_armed(false)?;
    println!("Paired (disarmed). Arm this Mac from your phone to enable git operations.");
    Ok(())
}
