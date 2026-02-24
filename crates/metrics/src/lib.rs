pub mod aggregator;
/// Metrics and observability
///
/// Provides event recording, aggregation, and export
pub mod event;
pub mod exporter;
pub mod prometheus;
pub mod recorder;

pub use aggregator::{MetricsAggregator, MetricsSnapshot};
pub use event::BotEvent;
pub use exporter::MetricsExporter;
pub use prometheus::PrometheusExporter;
pub use recorder::EventRecorder;
