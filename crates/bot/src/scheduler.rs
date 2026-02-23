/// Task scheduler for bot operations
///
/// Provides tokio-based task scheduling with support for:
/// - One-time tasks
/// - Recurring tasks
/// - Delayed execution

use crate::{BotError, Result};
use tokio::time::{Duration, Instant, interval_at, sleep};
use std::sync::Arc;
use parking_lot::Mutex;

/// A scheduled task
pub struct Task {
    /// Task identifier
    pub id: String,

    /// Task type
    pub kind: TaskKind,

    /// Task function
    pub func: Arc<dyn Fn() -> tokio::task::JoinHandle<()> + Send + Sync>,
}

/// Type of scheduled task
#[derive(Debug, Clone)]
pub enum TaskKind {
    /// Execute once immediately
    Immediate,

    /// Execute once after a delay
    Delayed { delay: Duration },

    /// Execute repeatedly at intervals
    Recurring { interval: Duration },
}

/// Task scheduler for managing bot operations
pub struct Scheduler {
    /// Active tasks
    tasks: Arc<Mutex<Vec<Task>>>,

    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    /// Schedule a one-time immediate task
    pub fn schedule_immediate<F>(&self, id: String, func: F) -> Result<()>
    where
        F: Fn() -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        let task = Task {
            id,
            kind: TaskKind::Immediate,
            func: Arc::new(func),
        };

        self.tasks.lock().push(task);
        Ok(())
    }

    /// Schedule a one-time delayed task
    pub fn schedule_delayed<F>(&self, id: String, delay: Duration, func: F) -> Result<()>
    where
        F: Fn() -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        let task = Task {
            id,
            kind: TaskKind::Delayed { delay },
            func: Arc::new(func),
        };

        self.tasks.lock().push(task);
        Ok(())
    }

    /// Schedule a recurring task
    pub fn schedule_recurring<F>(&self, id: String, interval_duration: Duration, func: F) -> Result<()>
    where
        F: Fn() -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        let task = Task {
            id,
            kind: TaskKind::Recurring { interval: interval_duration },
            func: Arc::new(func),
        };

        self.tasks.lock().push(task);
        Ok(())
    }

    /// Run all scheduled tasks
    pub async fn run(&self) -> Result<()> {
        let tasks = self.tasks.lock().clone();

        for task in tasks {
            if *self.shutdown.lock() {
                break;
            }

            match task.kind {
                TaskKind::Immediate => {
                    (task.func)();
                }
                TaskKind::Delayed { delay } => {
                    let func = task.func.clone();
                    let shutdown = self.shutdown.clone();

                    tokio::spawn(async move {
                        sleep(delay).await;
                        if !*shutdown.lock() {
                            func();
                        }
                    });
                }
                TaskKind::Recurring { interval: interval_duration } => {
                    let func = task.func.clone();
                    let shutdown = self.shutdown.clone();

                    tokio::spawn(async move {
                        let start = Instant::now() + interval_duration;
                        let mut ticker = interval_at(start, interval_duration);

                        loop {
                            ticker.tick().await;

                            if *shutdown.lock() {
                                break;
                            }

                            func();
                        }
                    });
                }
            }
        }

        Ok(())
    }

    /// Shutdown the scheduler
    pub fn shutdown(&self) {
        *self.shutdown.lock() = true;
    }

    /// Check if scheduler is shutdown
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.lock()
    }

    /// Get number of scheduled tasks
    pub fn task_count(&self) -> usize {
        self.tasks.lock().len()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_immediate_task() {
        let scheduler = Scheduler::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        scheduler
            .schedule_immediate("test-task".to_string(), move || {
                let c = counter_clone.clone();
                tokio::spawn(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
            })
            .unwrap();

        scheduler.run().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_delayed_task() {
        let scheduler = Scheduler::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        scheduler
            .schedule_delayed(
                "delayed-task".to_string(),
                Duration::from_millis(100),
                move || {
                    let c = counter_clone.clone();
                    tokio::spawn(async move {
                        c.fetch_add(1, Ordering::SeqCst);
                    })
                },
            )
            .unwrap();

        scheduler.run().await.unwrap();

        // Check before delay
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Check after delay
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_recurring_task() {
        let scheduler = Scheduler::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        scheduler
            .schedule_recurring(
                "recurring-task".to_string(),
                Duration::from_millis(50),
                move || {
                    let c = counter_clone.clone();
                    tokio::spawn(async move {
                        c.fetch_add(1, Ordering::SeqCst);
                    })
                },
            )
            .unwrap();

        scheduler.run().await.unwrap();

        // Wait for multiple intervals
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Should have executed at least 3 times
        assert!(counter.load(Ordering::SeqCst) >= 3);

        scheduler.shutdown();
    }

    #[test]
    fn test_task_count() {
        let scheduler = Scheduler::new();

        scheduler
            .schedule_immediate("task1".to_string(), || {
                tokio::spawn(async {})
            })
            .unwrap();

        scheduler
            .schedule_delayed("task2".to_string(), Duration::from_secs(1), || {
                tokio::spawn(async {})
            })
            .unwrap();

        assert_eq!(scheduler.task_count(), 2);
    }
}
