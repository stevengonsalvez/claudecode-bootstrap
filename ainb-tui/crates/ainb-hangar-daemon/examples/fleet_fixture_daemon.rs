//! Real Fleet daemon fixture for the macOS RPC contract tests.
//!
//! The process owns an isolated `AINB_HANGAR_HOME` supplied by its test parent.
//! It creates the real SQLite store, daemon token, Unix socket server, durable
//! Fleet revision log, and live event broker. JSON commands on stdin only inject
//! provider hook observations; they never emulate an RPC response or Fleet state.

use std::io::{BufRead as _, Write as _};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use ainb_hangar_daemon::events::EventBroker;
use ainb_hangar_daemon::fleet::{self, HookObservation};
use ainb_hangar_daemon::rpc::{self, DaemonHealth};
use ainb_hangar_store::Store;
use ainb_hangar_store::repo::fleet_provider_event::{
    FleetProviderEventRepo, NewFleetProviderEvent,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Command {
    Seed {
        event_id: String,
        #[serde(default = "default_provider")]
        provider: String,
        #[serde(default = "default_session_id")]
        session_id: String,
        #[serde(default = "default_event_type")]
        event_type: String,
        #[serde(default = "default_cwd")]
        cwd: String,
        #[serde(default)]
        payload: Value,
        #[serde(default)]
        observed_at: Option<i64>,
    },
    /// Commit one ACP transcript chunk and wake the transcript forwarders.
    ///
    /// The transcript's only production writer is the ACP pool's store writer,
    /// which needs a live adapter subprocess, so a fixture that stopped at
    /// `apply_hook` could prove nothing about `fleet/transcript_subscribe`: a
    /// client would have no way to make a chunk exist. This writes the SAME row
    /// the pool writes (`source='acp'`, through the real repo) and rings the
    /// SAME bell (`emit_transcript_order`); it does not emulate the RPC or the
    /// forwarder, both of which stay the daemon's own code under test.
    SeedTranscript {
        event_id: String,
        session_key: String,
        #[serde(default = "default_transcript_event_type")]
        event_type: String,
        #[serde(default = "default_transcript_provider")]
        provider: String,
        #[serde(default)]
        payload: Value,
        #[serde(default)]
        observed_at: Option<i64>,
    },
    Shutdown,
}

fn default_provider() -> String {
    "claude".to_string()
}
fn default_session_id() -> String {
    "fixture-session".to_string()
}
fn default_event_type() -> String {
    "SessionStart".to_string()
}
fn default_cwd() -> String {
    "/fixture".to_string()
}
fn default_transcript_event_type() -> String {
    "acp.message".to_string()
}
fn default_transcript_provider() -> String {
    "claude-agent-acp".to_string()
}

/// The `fleet_provider_event.source` the ACP pool writes transcript rows under.
///
/// Named rather than repeated as a literal: the prune path selects on it, so a
/// fixture that drifted from the production writer would seed rows the daemon
/// can read but never reclaim.
const ACP_TRANSCRIPT_SOURCE: &str = "acp";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let home = fixture_home()?;
    let store = Store::open_in(&home).await?;
    rpc::auth::ensure_socket_token(store.pool(), &home).await?;
    let socket = rpc::socket_path_in(&home);
    let listener = rpc::bind(&socket)?;
    let broker = EventBroker::new();
    let sink = broker.sink();
    let health = DaemonHealth {
        socket_path: socket.to_string_lossy().into_owned(),
        pid: std::process::id(),
        started_at: Instant::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        stats: Arc::new(ainb_hangar_daemon::health_stats::HealthStats::default()),
    };
    tokio::spawn(rpc::serve(listener, store.pool().clone(), health, broker));

    let mut next_observed_at = 1_700_000_000_000_i64;
    'commands: loop {
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            let command: Command = match serde_json::from_str(&line) {
                Ok(command) => command,
                Err(error) => {
                    println!(
                        "{}",
                        serde_json::json!({ "ok": false, "error": error.to_string() })
                    );
                    std::io::stdout().flush()?;
                    continue;
                }
            };
            match command {
                Command::Seed {
                    event_id,
                    provider,
                    session_id,
                    event_type,
                    cwd,
                    payload,
                    observed_at,
                } => {
                    let observed_at = observed_at.unwrap_or_else(|| {
                        next_observed_at += 1;
                        next_observed_at
                    });
                    match fleet::apply_hook(
                        store.pool(),
                        &sink,
                        HookObservation {
                            event_id: event_id.clone(),
                            provider: &provider,
                            provider_session_id: &session_id,
                            cwd: &cwd,
                            event_type: &event_type,
                            payload: &payload,
                            observed_at,
                            transcript_model: None,
                        },
                    )
                    .await
                    {
                        Ok(result) => println!(
                            "{}",
                            serde_json::json!({
                                "ok": true,
                                "event_id": event_id,
                                "revision": result.revision,
                                "duplicate": result.duplicate,
                            })
                        ),
                        Err(error) => println!(
                            "{}",
                            serde_json::json!({ "ok": false, "error": error.to_string() })
                        ),
                    }
                    std::io::stdout().flush()?;
                }
                Command::SeedTranscript {
                    event_id,
                    session_key,
                    event_type,
                    provider,
                    payload,
                    observed_at,
                } => {
                    let observed_at = observed_at.unwrap_or_else(|| {
                        next_observed_at += 1;
                        next_observed_at
                    });
                    let row = FleetProviderEventRepo::append(
                        store.pool(),
                        &NewFleetProviderEvent {
                            event_id: event_id.clone(),
                            provider,
                            // The source the ACP pool writes its transcript
                            // rows under. NEITHER transcript read filters on
                            // it (both select by `session_key` alone), so this
                            // is about the row being the same shape the
                            // production writer produces, not about being
                            // visible: a fixture that seeded some other source
                            // would be testing a row the daemon never writes.
                            //
                            // The prune path DOES filter `source = 'acp'`, so
                            // a row written under another source would also be
                            // unprunable, which is the second reason to match.
                            source: ACP_TRANSCRIPT_SOURCE.to_string(),
                            session_key: Some(session_key.clone()),
                            provider_session_id: None,
                            observed_at,
                            received_at: observed_at,
                            event_type,
                            raw_payload: payload.to_string(),
                        },
                    )
                    .await;
                    match row {
                        Ok(row) => {
                            // AFTER the row is durable, never before: a
                            // forwarder woken early reads the log and finds
                            // nothing, which is the shape of a flake.
                            sink.emit_transcript_order(&session_key, row.ingest_order);
                            println!(
                                "{}",
                                serde_json::json!({
                                    "ok": true,
                                    "event_id": event_id,
                                    "ingest_order": row.ingest_order,
                                })
                            );
                        }
                        Err(error) => println!(
                            "{}",
                            serde_json::json!({ "ok": false, "error": error.to_string() })
                        ),
                    }
                    std::io::stdout().flush()?;
                }
                Command::Shutdown => break 'commands,
            }
        }
        // XCTest may momentarily close its inherited stdin during test-host launch.
        // Keep the real daemon available for its readiness and socket proof instead
        // of turning that transport startup race into a clean fixture exit.
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

fn fixture_home() -> anyhow::Result<PathBuf> {
    let raw = std::env::var_os("AINB_HANGAR_HOME")
        .ok_or_else(|| anyhow::anyhow!("AINB_HANGAR_HOME is required for fixture isolation"))?;
    let home = PathBuf::from(raw);
    if !home.is_absolute() {
        anyhow::bail!("AINB_HANGAR_HOME must be an absolute isolated directory");
    }
    std::fs::create_dir_all(&home)?;
    Ok(home)
}
