//! Agent authentication (Phase 2a). The agent never self-signs: at pairing it
//! receives an opaque refresh token from the issuer and thereafter exchanges it
//! for a short-lived session JWT via `POST {site}/auth/refresh`. The session
//! token is presented to Convex through `ConvexClient::set_auth_callback`, whose
//! fetcher re-mints on connect + every reconnect (rotating the refresh token).

use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use convex::{AuthenticationToken, AuthTokenFetcher};
use tokio::sync::Mutex;

/// In-memory auth state: the durable refresh token (rotated on use) plus a cache
/// of the latest short-lived session JWT (never persisted).
struct State {
    refresh: String,
    cached_jwt: Option<String>,
    /// Conservative expiry of `cached_jwt` (set to ~just under the 1h TTL on mint).
    jwt_until: Option<Instant>,
}

/// Loaded pairing credentials + the issuer site origin + a shared HTTP client.
pub struct AgentAuth {
    site: String,
    http: reqwest::Client,
    state: Arc<Mutex<State>>,
    /// Set when a refresh is rejected (401) — the agent's credential was revoked
    /// (unpaired). The relay observes this and exits cleanly.
    revoked: Arc<AtomicBool>,
}

// Session JWTs are minted with a 1h TTL; treat the cache as valid for a bit less
// so we always re-mint with margin, and refresh when within 120s of that.
const JWT_CACHE_SECS: u64 = 3300;
const REFRESH_MARGIN: Duration = Duration::from_secs(120);

impl AgentAuth {
    /// Load persisted credentials. Returns None if the agent isn't paired yet
    /// (run `git-it-agent pair <code>` first) or CONVEX_SITE_URL is unset.
    pub fn load() -> Option<AgentAuth> {
        let site = std::env::var("CONVEX_SITE_URL").ok()?;
        let refresh = crate::repos::load_refresh_token()?;
        let http = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Some(AgentAuth {
            site: site.trim_end_matches('/').to_string(),
            http,
            state: Arc::new(Mutex::new(State { refresh, cached_jwt: None, jwt_until: None })),
            revoked: Arc::new(AtomicBool::new(false)),
        })
    }

    /// A flag the relay can poll to exit cleanly once the credential is revoked.
    pub fn revoked_flag(&self) -> Arc<AtomicBool> {
        self.revoked.clone()
    }

    /// Build the Convex auth fetcher. On `force_refresh` (reconnect) or when the
    /// cached JWT is within the margin of expiry it refreshes (rotating the
    /// refresh token); otherwise it returns the cached JWT with no network call.
    /// The network call happens OUTSIDE the lock; the lock is only held to read
    /// the refresh token and to write+persist the rotated one (no `.await` across
    /// the network while holding it).
    pub fn make_fetcher(&self) -> AuthTokenFetcher {
        let site = self.site.clone();
        let http = self.http.clone();
        let state = self.state.clone();
        let revoked = self.revoked.clone();
        Box::new(move |force_refresh: bool| {
            let site = site.clone();
            let http = http.clone();
            let state = state.clone();
            let revoked = revoked.clone();
            Box::pin(async move {
                // Reuse a still-valid cached JWT on a non-forced call.
                let refresh = {
                    let s = state.lock().await;
                    if !force_refresh {
                        if let (Some(jwt), Some(until)) = (&s.cached_jwt, s.jwt_until) {
                            if Instant::now() + REFRESH_MARGIN < until {
                                return Ok(AuthenticationToken::User(jwt.clone()));
                            }
                        }
                    }
                    s.refresh.clone()
                };
                let (token, rotated) = match http_refresh(&http, &site, &refresh).await {
                    Ok(v) => v,
                    Err(e) => {
                        if e.revoked {
                            revoked.store(true, Ordering::Relaxed);
                        }
                        return Err(e.into());
                    }
                };
                {
                    let mut s = state.lock().await;
                    s.refresh = rotated.clone();
                    s.cached_jwt = Some(token.clone());
                    s.jwt_until = Some(Instant::now() + Duration::from_secs(JWT_CACHE_SECS));
                    // Fail fast: the server already invalidated the old refresh
                    // token, so if we can't persist the rotated one the agent
                    // would be locked out on restart — refuse to proceed.
                    crate::repos::update_refresh_token(&rotated).map_err(|e| {
                        anyhow::anyhow!(
                            "could not persist rotated refresh token to agent.json ({e}); \
                             re-pair with `git-it-agent pair`"
                        )
                    })?;
                }
                Ok(AuthenticationToken::User(token))
            }) as Pin<Box<dyn Future<Output = anyhow::Result<AuthenticationToken>> + Send>>
        })
    }
}

/// Error from a refresh attempt; `revoked` marks a definitive 401 (re-pair needed)
/// vs a transient network/server error (retry on the next reconnect).
struct RefreshError {
    revoked: bool,
    msg: String,
}
impl From<RefreshError> for anyhow::Error {
    fn from(e: RefreshError) -> Self {
        anyhow::anyhow!(e.msg)
    }
}

/// Exchange a refresh token for a fresh session JWT (+ a rotated refresh token).
async fn http_refresh(
    http: &reqwest::Client,
    site: &str,
    refresh_token: &str,
) -> Result<(String, String), RefreshError> {
    let resp = http
        .post(format!("{site}/auth/refresh"))
        .json(&serde_json::json!({ "refreshToken": refresh_token }))
        .send()
        .await
        .map_err(|e| RefreshError { revoked: false, msg: format!("refresh request failed: {e}") })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(RefreshError {
            revoked: status.as_u16() == 401,
            msg: format!("session refresh failed: HTTP {status} — re-pair with `git-it-agent pair`"),
        });
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| RefreshError { revoked: false, msg: format!("bad refresh response: {e}") })?;
    let token = v.get("token").and_then(|x| x.as_str())
        .ok_or_else(|| RefreshError { revoked: false, msg: "refresh response missing token".into() })?;
    let rotated = v.get("refreshToken").and_then(|x| x.as_str())
        .ok_or_else(|| RefreshError { revoked: false, msg: "refresh response missing refreshToken".into() })?;
    Ok((token.to_string(), rotated.to_string()))
}

/// `git-it-agent pair <code>`: claim a phone-issued pairing code. Provisions this
/// Mac (DISARMED) under the phone's account and persists the credentials. Writes
/// the refresh token + the pairing-time device id AND the local disarmed flag in
/// one atomic config write, so there is never a window where auth exists without
/// the disarm gate engaged.
pub async fn run_pairing(code: &str) -> anyhow::Result<()> {
    dotenvy::from_path(crate::repos::env_path()).ok();
    let site = std::env::var("CONVEX_SITE_URL")
        .map_err(|_| anyhow::anyhow!("set CONVEX_SITE_URL to the deployment's .convex.site origin"))?;
    let site = site.trim_end_matches('/');
    let device_id = crate::repos::device_id();
    let device_name = crate::repos::device_name();
    let resp = reqwest::Client::builder().build()?
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
    // creds + the pairing-time device id + armed:false, atomically.
    crate::repos::complete_pairing(refresh, account, &device_id)?;
    println!("Paired (disarmed). Arm this Mac from your phone to enable git operations.");
    Ok(())
}
