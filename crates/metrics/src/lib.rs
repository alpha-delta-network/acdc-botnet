/// Metrics and observability
///
/// Provides event recording, aggregation, and export

pub mod event;
pub mod recorder;
pub mod aggregator;
pub mod exporter;

pub use event::BotEvent;
pub use recorder::EventRecorder;
pub use aggregator::MetricsAggregator;
pub use exporter::MetricsExporter;
