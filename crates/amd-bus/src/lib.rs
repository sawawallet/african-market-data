//! NATS JetStream transport for normalised market data events.

pub mod subject;

use amd_core::{Bar, ExchangeCode, InstrumentId, Quote};
use async_nats::jetstream::{self, stream::RetentionPolicy};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

pub use subject::Kind;

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("connect failed: {0}")]
    Connect(String),
    #[error("publish failed: {0}")]
    Publish(String),
    #[error("subscribe failed: {0}")]
    Subscribe(String),
    #[error("stream setup failed: {0}")]
    Stream(String),
    #[error("could not encode event: {0}")]
    Encode(#[from] serde_json::Error),
}

/// The envelope every event travels in.
///
/// Versioned at the envelope rather than only in the subject, so a consumer
/// that receives an unexpected shape can say so precisely instead of failing to
/// deserialize with no context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Quote(Quote),
    Bar(Bar),
    /// Venue session transitions, so downstream consumers need not each run
    /// their own copy of the calendar.
    Status {
        exchange: ExchangeCode,
        state: String,
    },
}

impl Event {
    pub fn subject(&self) -> String {
        match self {
            Event::Quote(q) => subject::for_instrument(&q.instrument, Kind::Quote),
            Event::Bar(b) => subject::for_instrument(&b.instrument, Kind::Bar),
            Event::Status { exchange, .. } => subject::for_venue(*exchange, Kind::Status),
        }
    }

    pub fn instrument(&self) -> Option<&InstrumentId> {
        match self {
            Event::Quote(q) => Some(&q.instrument),
            Event::Bar(b) => Some(&b.instrument),
            Event::Status { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BusConfig {
    pub url: String,
    /// JetStream stream name. One stream covers every venue; per-venue
    /// isolation is achieved with leaf nodes, not with separate streams.
    pub stream: String,
    /// How long the bus retains events for replay. This is a buffer for
    /// consumer outages, not the archive — durable history lives in ClickHouse.
    pub max_age_secs: u64,
}

impl Default for BusConfig {
    fn default() -> Self {
        BusConfig {
            url: "nats://127.0.0.1:4222".to_owned(),
            stream: "AMD".to_owned(),
            max_age_secs: 24 * 3600,
        }
    }
}

#[derive(Clone)]
pub struct Bus {
    js: jetstream::Context,
    client: async_nats::Client,
}

impl Bus {
    pub async fn connect(cfg: &BusConfig) -> Result<Self, BusError> {
        let client = async_nats::connect(&cfg.url)
            .await
            .map_err(|e| BusError::Connect(e.to_string()))?;
        let js = jetstream::new(client.clone());

        js.get_or_create_stream(jetstream::stream::Config {
            name: cfg.stream.clone(),
            subjects: vec![subject::all()],
            // Limits, not WorkQueue: multiple independent consumers (archiver,
            // API fan-out, analytics) each need their own view of the same
            // events, and a work queue would let one consume another's.
            retention: RetentionPolicy::Limits,
            max_age: std::time::Duration::from_secs(cfg.max_age_secs),
            ..Default::default()
        })
        .await
        .map_err(|e| BusError::Stream(e.to_string()))?;

        info!(url = %cfg.url, stream = %cfg.stream, "bus connected");
        Ok(Bus { js, client })
    }

    /// Publish and await the JetStream ack.
    ///
    /// Awaiting the ack is deliberate: an ingestor that publishes without
    /// confirmation cannot tell a full stream from a healthy one, and silently
    /// drops data during exactly the incident you need the data for.
    pub async fn publish(&self, event: &Event) -> Result<(), BusError> {
        let subject = event.subject();
        let payload = serde_json::to_vec(event)?;
        let ack = self
            .js
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;
        ack.await.map_err(|e| BusError::Publish(e.to_string()))?;
        debug!(%subject, "published");
        Ok(())
    }

    pub async fn publish_all(&self, events: &[Event]) -> Result<usize, BusError> {
        let mut n = 0;
        for e in events {
            self.publish(e).await?;
            n += 1;
        }
        Ok(n)
    }

    /// Ephemeral core-NATS subscription: live events only, no replay.
    ///
    /// This is what the API's SSE fan-out uses. A browser reconnecting does not
    /// want yesterday's ticks replayed at it; it wants a fresh snapshot from
    /// the store plus the live stream from here.
    ///
    /// Returns a `'static` boxed stream rather than an opaque type: under the
    /// 2024 edition an RPIT captures the borrows of `&self` and `subject`, and
    /// callers need to hold this across an await in a handler that has already
    /// dropped both.
    pub async fn subscribe(
        &self,
        subject: &str,
    ) -> Result<futures::stream::BoxStream<'static, Event>, BusError> {
        let sub = self
            .client
            .subscribe(subject.to_owned())
            .await
            .map_err(|e| BusError::Subscribe(e.to_string()))?;
        Ok(Box::pin(sub.filter_map(|msg| async move {
            match serde_json::from_slice::<Event>(&msg.payload) {
                Ok(ev) => Some(ev),
                Err(e) => {
                    tracing::warn!(subject = %msg.subject, error = %e, "undecodable event dropped");
                    None
                }
            }
        })))
    }

    /// Durable JetStream consumer: survives restarts and replays what it missed.
    ///
    /// Used by the archiver, which must not lose events across a deploy.
    pub async fn durable_consumer(
        &self,
        stream: &str,
        durable_name: &str,
        filter: &str,
    ) -> Result<jetstream::consumer::PullConsumer, BusError> {
        let stream = self
            .js
            .get_stream(stream)
            .await
            .map_err(|e| BusError::Stream(e.to_string()))?;
        stream
            .get_or_create_consumer(
                durable_name,
                jetstream::consumer::pull::Config {
                    durable_name: Some(durable_name.to_owned()),
                    filter_subject: filter.to_owned(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| BusError::Stream(e.to_string()))
    }

    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }
}
