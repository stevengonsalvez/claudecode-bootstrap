//! The write-transaction timer actually emits the line an operator is told to
//! grep for.
//!
//! The instrument exists to name the code path that holds the single `SQLite`
//! writer through a contention episode. That promise has two halves, and a test
//! that only checks the type compiles proves neither: the line must be EMITTED
//! when a transaction runs long, and it must CARRY the literal an operator
//! greps for plus the call site that identifies the culprit.
//!
//! A short transaction emitting nothing is the other half: at the daemon's 1 Hz
//! tick almost every transaction is milliseconds long, and an instrument that
//! logged those would drown the signal it exists to surface.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ainb_hangar_store::write_tx_timer::WriteTxTimer;
use tracing::field::{Field, Visit};
use tracing::subscriber::set_default;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};

/// One captured event: its target plus every field rendered to a string.
#[derive(Debug, Clone, Default)]
struct CapturedEvent {
    target: String,
    fields: Vec<(String, String)>,
}

impl CapturedEvent {
    /// Everything an operator could match on, concatenated the way a log line
    /// presents it.
    fn greppable(&self) -> String {
        let body = self
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{} {body}", self.target)
    }
}

type EventLog = Arc<Mutex<Vec<CapturedEvent>>>;

struct CollectEvents {
    log: EventLog,
}

struct FieldCollector<'a>(&'a mut Vec<(String, String)>);

impl Visit for FieldCollector<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_string(), format!("{value:?}")));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_string(), value.to_string()));
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.push((field.name().to_string(), value.to_string()));
    }
}

impl<S: tracing::Subscriber> Layer<S> for CollectEvents {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut captured = CapturedEvent {
            target: event.metadata().target().to_string(),
            fields: Vec::new(),
        };
        event.record(&mut FieldCollector(&mut captured.fields));
        self.log.lock().expect("event log").push(captured);
    }
}

/// Run `body` with an event-capturing subscriber installed, returning what it
/// observed.
fn capture(body: impl FnOnce()) -> Vec<CapturedEvent> {
    let log: EventLog = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(CollectEvents { log: log.clone() });
    {
        let _guard = set_default(subscriber);
        body();
    }
    let events = log.lock().expect("event log").clone();
    events
}

/// A long transaction names itself, with the string operators are told to grep.
#[test]
fn a_long_transaction_emits_the_documented_grep_marker() {
    let events = capture(|| {
        let timer =
            WriteTxTimer::start_with_threshold("crates/example/src/thing.rs:42", Duration::ZERO);
        drop(timer);
    });

    let held: Vec<_> = events
        .iter()
        .filter(|e| e.greppable().contains("hangar.writetx.held"))
        .collect();
    assert_eq!(
        held.len(),
        1,
        "a transaction over the threshold must emit exactly one report, got: {events:#?}"
    );

    let line = held[0].greppable();
    assert!(
        line.contains("crates/example/src/thing.rs:42"),
        "the report must name the call site, or it identifies no culprit: {line}"
    );
    assert!(
        line.contains("held_secs"),
        "the report must carry how long it held: {line}"
    );
}

/// A short transaction is silent.
#[test]
fn a_short_transaction_emits_nothing() {
    let events = capture(|| {
        let timer = WriteTxTimer::start_with_threshold(
            "crates/example/src/thing.rs:7",
            Duration::from_secs(3600),
        );
        drop(timer);
    });

    let held = events.iter().filter(|e| e.greppable().contains("hangar.writetx.held")).count();
    assert_eq!(
        held, 0,
        "a transaction inside the threshold must be silent, or the instrument drowns \
         the signal it exists to surface: {events:#?}"
    );
}
