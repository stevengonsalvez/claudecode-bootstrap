//! A chat WRITE must leave the surface on the conversation it was written in.
//!
//! Driven against a REAL daemon on an isolated socket, through the REAL
//! [`ChatHost`] worker: dispatch the intent, tick the host, read the surface.
//! Nothing here constructs a `ChatSnapshot` and asserts on it; the snapshots
//! this reads are the ones `fleet/channel_list` + `fleet/message_list` actually
//! produced.
//!
//! The trap being pinned: every write in `chat_host` ends by paging, and the
//! copilot page RESOLVES its channel when it is handed no scope. Resolution is
//! newest-wins (matching the daemon's own `newest_of_kind`), so a page with no
//! scope lands on whatever copilot channel is newest, not on the one the
//! operator is reading. A second copilot channel is a thing the CLI mints on
//! demand and a race mints by accident, and when one exists a confirm answer or
//! a turn cancel would swap the pane's whole conversation out underneath it.
//!
//! Not proven here: a SUCCESSFUL `fleet/action` interrupt. `Delivered` needs a
//! live ACP session in the pool, which needs a real adapter process; without
//! one the daemon answers `Unknown` and the cancel reports a refusal. The scope
//! a cancel pages on is the same either way, which is what this file is about.

use std::time::{Duration, Instant};

use ainb::fleet::chat_host::ChatHost;
use ainb_hangar_proto::fleet::{FleetConfirmAnswer, FleetConfirmAnswerParams};
use ainb_hangar_store::repo::fleet_chat::{
    FleetChannelRepo, FleetChannelRow, FleetConfirmRepo, FleetConfirmRow,
};
use ainb_hangar_store::repo::fleet_message::{FleetMessageRepo, NewFleetMessage};
use ainb_plugin_hangar::screen::fleet_chat::ChatIntent;

#[path = "support/fleet_hangar.rs"]
mod fleet_hangar;

use fleet_hangar::{EnvGuard, FleetHangar};

/// `$AINB_HANGAR_HOME` is process-wide and both tests here set it. Cargo runs a
/// test binary's tests on threads of ONE process, so without this the second
/// test's home would be dialled by the first test's worker.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The clock every tick is handed.
///
/// Constant on purpose: `chat_tick` stamps `last_poll_ms` and then refuses to
/// poll again inside the interval, so a fixed clock means the surface dispatches
/// exactly ONE refresh of its own and the rest of the traffic is the intent
/// under test.
const NOW_MS: i64 = 5_000_000;

/// Long enough for four local RPCs on a cold sqlite file, short enough that a
/// hung worker fails the test rather than the job.
const SETTLE: Duration = Duration::from_secs(20);

struct Fixture {
    _home: tempfile::TempDir,
    _env: EnvGuard,
    hangar: FleetHangar,
    _lock: std::sync::MutexGuard<'static, ()>,
}

fn fixture(prefix: &str) -> Fixture {
    let lock = ENV_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let home = tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in("/tmp")
        .expect("home tempdir");
    let hangar_home = home.path().join("hangar-home");
    std::fs::create_dir_all(&hangar_home).expect("create isolated hangar home");
    let hangar = FleetHangar::start(&hangar_home);
    let env = EnvGuard::set("AINB_HANGAR_HOME", &hangar_home);
    Fixture {
        _home: home,
        _env: env,
        hangar,
        _lock: lock,
    }
}

/// Mint one copilot channel directly in the store, at an exact `created_at`.
///
/// Written through the repo rather than the RPC because the ORDER of the two
/// channels is the whole point and `fleet/channel_create` stamps its own clock.
fn seed_copilot_channel(hangar: &FleetHangar, id: &str, created_at: i64) -> String {
    let scope = format!("channel:{id}");
    hangar.block_on(async {
        FleetChannelRepo::insert(
            hangar.pool(),
            &FleetChannelRow {
                id: id.to_string(),
                kind: "copilot".to_string(),
                name: "copilot".to_string(),
                scope_key: scope.clone(),
                recipients: Vec::new(),
                copilot_mode: ainb_hangar_proto::fleet::FleetCopilotMode::default()
                    .as_str()
                    .to_string(),
                created_at,
            },
        )
        .await
        .expect("seed copilot channel");
    });
    scope
}

fn seed_message(hangar: &FleetHangar, scope: &str, id: &str, body: &str, created_at: i64) {
    hangar.block_on(async {
        FleetMessageRepo::insert_message(
            hangar.pool(),
            &NewFleetMessage {
                id: id.to_string(),
                request_id: None,
                request_fingerprint: None,
                scope_key: scope.to_string(),
                origin_message_id: None,
                sender: "operator".to_string(),
                kind: "user".to_string(),
                body: body.to_string(),
                created_at,
            },
        )
        .await
        .expect("seed chat message");
    });
}

/// Tick until `settled` reports the next page has landed, or fail loudly.
fn tick_until(host: &mut ChatHost, what: &str, settled: impl Fn(&ChatHost) -> bool) {
    let deadline = Instant::now() + SETTLE;
    loop {
        host.tick(NOW_MS);
        if settled(host) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what}: nothing landed within {SETTLE:?}; surface is {:?} on scope {:?}",
            host.state().status(),
            host.state().scope_key()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Open the copilot surface for real and leave it on the only channel there is.
fn open_on(hangar: &FleetHangar, scope: &str) -> ChatHost {
    let mut host = ChatHost::copilot();
    tick_until(&mut host, "the cold open", |host| {
        host.state().scope_key() == Some(scope)
    });
    assert_eq!(
        host.state().messages().len(),
        1,
        "the cold open must have paged the seeded line"
    );
    let _ = hangar;
    host
}

/// A write dispatched here must page the scope the surface is ON.
///
/// The evidence is a message inserted into that channel AFTER the cold open: a
/// page of the right channel finds two rows, a page of the newer channel finds
/// none, so the predicate below distinguishes "landed correctly", "landed on the
/// wrong conversation" and "never landed" without a timing window.
fn assert_write_stays_on_scope(prefix: &str, intent_for: impl Fn(&FleetHangar) -> ChatIntent) {
    let fixture = fixture(prefix);
    let hangar = &fixture.hangar;

    let reading = seed_copilot_channel(hangar, "01J0CHANNELREADING", 1_700_000_000_000);
    seed_message(
        hangar,
        &reading,
        "01J0MSGREADINGONE",
        "the line the operator is reading",
        1_700_000_000_000,
    );

    let mut host = open_on(hangar, &reading);

    // The second copilot channel, minted AFTER the surface resolved its own and
    // therefore newer. `ainb fleet chat --kind copilot` mints one on demand, and
    // `chat_page_blocking` documents a race minting one by accident. Left EMPTY
    // so the two outcomes are told apart by row count alone: a page of the
    // channel the operator is reading has two rows, a page of this one has none,
    // and "still one" is a page that has not landed yet.
    seed_copilot_channel(hangar, "01J0CHANNELNEWER", 1_700_000_100_000);
    seed_message(
        hangar,
        &reading,
        "01J0MSGREADINGTWO",
        "a second line on the same conversation",
        1_700_000_200_000,
    );

    host.dispatch(intent_for(hangar));
    tick_until(&mut host, "the write's page", |host| {
        host.state().messages().len() != 1
    });

    assert_eq!(
        host.state().scope_key(),
        Some(reading.as_str()),
        "the write paged {:?}, swapping the operator's conversation for another",
        host.state().scope_key()
    );
    let bodies: Vec<&str> = host.state().messages().iter().map(|row| row.body.as_str()).collect();
    assert_eq!(
        bodies,
        vec![
            "the line the operator is reading",
            "a second line on the same conversation"
        ],
        "the write's page shows a timeline the operator was not reading"
    );
    // The page RESOLVED an existing channel; it did not mint a third. Asserted
    // because the failure mode this file guards is a silent switch between
    // conversations that both exist, not a runaway `fleet/channel_create`.
    let channels = hangar.block_on(FleetChannelRepo::list(hangar.pool())).expect("list channels");
    assert_eq!(channels.len(), 2, "a write minted a channel of its own");
}

/// Answering a confirm card must not move the pane off its conversation.
#[test]
fn answering_a_confirm_pages_the_conversation_it_was_answered_in() {
    assert_write_stays_on_scope("chat-host-confirm-scope-", |hangar| {
        // A REAL open card on the channel the surface is reading, so the answer
        // is one the daemon actually resolves rather than a refusal that would
        // exercise a different arm.
        hangar.block_on(async {
            FleetConfirmRepo::insert(
                hangar.pool(),
                &FleetConfirmRow {
                    confirm_id: "01J0CARDREADING".to_string(),
                    scope_key: "channel:01J0CHANNELREADING".to_string(),
                    tool: "kill".to_string(),
                    arguments: r#"{"session":"claude:one"}"#.to_string(),
                    target_session_key: Some("claude:one".to_string()),
                    state: "open".to_string(),
                    edited_arguments: None,
                    created_at: 1_700_000_000_000,
                    expires_at: 4_000_000_000_000,
                    answered_at: None,
                },
            )
            .await
            .expect("seed one open confirm card");
        });
        ChatIntent::ConfirmAnswer(FleetConfirmAnswerParams {
            confirm_id: "01J0CARDREADING".to_string(),
            answer: FleetConfirmAnswer::Approve,
        })
    });
}

/// Cancelling a turn must not move the pane off its conversation either.
#[test]
fn cancelling_a_turn_pages_the_conversation_it_was_cancelled_in() {
    assert_write_stays_on_scope("chat-host-cancel-scope-", |_hangar| {
        ChatIntent::CancelTurn {
            session_keys: vec!["claude:one".to_string()],
        }
    });
}
