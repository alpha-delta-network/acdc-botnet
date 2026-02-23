# Dynamic Resource Management (Enhancement Proposal)

**Status**: Proposed Enhancement
**Priority**: P1 (Critical for Production)
**Estimated Effort**: 1-2 weeks

---

## Problem Statement

Current implementation uses static `--max-bots` limits without real-time resource awareness. This can:
- Impact other system processes (validators, nodes, databases)
- Cause OOM kills or CPU starvation
- Provide no protection against resource exhaustion
- Require manual tuning for each host

**Goal**: Operate as many bots as possible within system constraints without impacting other functions.

---

## Proposed Architecture

### 1. Resource Monitor (Worker-Side)

```rust
// crates/distributed/src/resource_monitor.rs
pub struct ResourceMonitor {
    interval: Duration,
    thresholds: ResourceThresholds,
    current: Arc<RwLock<ResourceSnapshot>>,
}

pub struct ResourceThresholds {
    cpu_max: f32,         // 80% - soft limit
    cpu_critical: f32,    // 95% - hard limit
    memory_max: f32,      // 85% - soft limit
    memory_critical: f32, // 95% - hard limit
    io_wait_max: f32,     // 20% - I/O bottleneck
}

pub struct ResourceSnapshot {
    timestamp: SystemTime,
    cpu_percent: f32,          // 0-100%
    memory_percent: f32,       // 0-100%
    memory_used_bytes: u64,
    memory_available_bytes: u64,
    io_wait_percent: f32,      // I/O wait time
    load_average_1m: f32,      // 1-minute load average
    open_file_descriptors: u32,
    network_connections: u32,
}

impl ResourceMonitor {
    /// Sample system resources every N seconds
    pub async fn monitor_loop(&self, throttle_tx: mpsc::Sender<ThrottleSignal>) {
        loop {
            let snapshot = self.sample_resources().await;
            let signal = self.evaluate_throttle(&snapshot);
            
            if let Some(sig) = signal {
                throttle_tx.send(sig).await?;
            }
            
            tokio::time::sleep(self.interval).await;
        }
    }
    
    fn sample_resources(&self) -> ResourceSnapshot {
        // Use procfs on Linux, sysinfo crate cross-platform
        // Read: /proc/stat (CPU), /proc/meminfo (memory), /proc/loadavg
    }
    
    fn evaluate_throttle(&self, snap: &ResourceSnapshot) -> Option<ThrottleSignal> {
        if snap.cpu_percent > self.thresholds.cpu_critical 
           || snap.memory_percent > self.thresholds.memory_critical {
            return Some(ThrottleSignal::EmergencyStop);
        }
        
        if snap.cpu_percent > self.thresholds.cpu_max 
           || snap.memory_percent > self.thresholds.memory_max {
            return Some(ThrottleSignal::SlowDown);
        }
        
        // Good state - can spawn more
        if snap.cpu_percent < 60.0 && snap.memory_percent < 70.0 {
            return Some(ThrottleSignal::SpeedUp);
        }
        
        None // Maintain current rate
    }
}
```

### 2. Throttle Controller (Worker-Side)

```rust
pub enum ThrottleSignal {
    SpeedUp,         // Resources available, can spawn more
    SlowDown,        // Approaching limits, reduce spawn rate
    EmergencyStop,   // Critical threshold, pause all spawning
    Resume,          // Resources recovered, resume normal operation
}

pub struct ThrottleController {
    max_bots: u32,              // Hard limit from CLI
    current_bots: AtomicU32,    // Active bot count
    target_bots: AtomicU32,     // Dynamic target (≤ max_bots)
    spawn_rate: AtomicU32,      // Bots/second spawn rate
}

impl ThrottleController {
    pub async fn apply_throttle(&self, signal: ThrottleSignal) {
        match signal {
            ThrottleSignal::EmergencyStop => {
                // Pause all new bot spawning
                self.spawn_rate.store(0, Ordering::SeqCst);
                // Consider terminating idle bots to free resources
                self.terminate_idle_bots(10).await;
            }
            
            ThrottleSignal::SlowDown => {
                // Reduce spawn rate by 50%
                let current = self.spawn_rate.load(Ordering::SeqCst);
                self.spawn_rate.store(current / 2, Ordering::SeqCst);
                // Reduce target by 10%
                let target = self.target_bots.load(Ordering::SeqCst);
                self.target_bots.store((target * 9) / 10, Ordering::SeqCst);
            }
            
            ThrottleSignal::SpeedUp => {
                // Can handle more load
                let current = self.target_bots.load(Ordering::SeqCst);
                let max = self.max_bots;
                if current < max {
                    // Increase target by 10%, up to max
                    let new_target = std::cmp::min((current * 11) / 10, max);
                    self.target_bots.store(new_target, Ordering::SeqCst);
                }
            }
            
            ThrottleSignal::Resume => {
                // Restore to configured max
                self.target_bots.store(self.max_bots, Ordering::SeqCst);
            }
        }
    }
}
```

### 3. Configuration (CLI Parameters)

```bash
acdc-botnet worker start \
  --coordinator ci.ac-dc.network:50051 \
  --max-bots 300 \
  --cpu-threshold 80 \        # Soft limit (start throttling)
  --cpu-critical 95 \          # Hard limit (emergency stop)
  --memory-threshold 85 \      # Soft limit
  --memory-critical 95 \       # Hard limit
  --resource-check-interval 5s \  # Monitor every 5s
  --auto-scale                 # Enable dynamic scaling
```

### 4. OS-Level Isolation (Optional)

```bash
# Run worker in cgroup with resource limits
sudo cgcreate -g cpu,memory:acdc-botnet
sudo cgset -r cpu.shares=512 acdc-botnet    # 50% CPU (1024 = 100%)
sudo cgset -r memory.limit_in_bytes=8G acdc-botnet

# Run with nice value (lower priority)
nice -n 10 acdc-botnet worker start ...

# Or use systemd with resource controls
[Service]
CPUQuota=80%
MemoryMax=8G
Nice=10
IOWeight=100  # Lower I/O priority
```

---

## Implementation Plan

### Phase 1: Basic Monitoring (1 week)

**Files to create**:
- `crates/distributed/src/resource_monitor.rs`
- `crates/distributed/src/throttle.rs`

**Dependencies**:
```toml
[dependencies]
sysinfo = "0.30"      # Cross-platform system info
procfs = "0.16"       # Linux /proc filesystem (optional, more efficient)
```

**Tasks**:
1. Implement ResourceMonitor with CPU/memory sampling
2. Add throttle signal evaluation
3. Wire into worker startup
4. Add CLI parameters
5. Test with resource-exhaustion scenario

### Phase 2: Dynamic Throttling (3-5 days)

**Tasks**:
1. Implement ThrottleController
2. Connect monitor → controller → bot spawner
3. Add gradual scale-up/scale-down
4. Test with sustained-load-48h scenario
5. Verify no impact on co-located services

### Phase 3: Advanced Features (3-5 days)

**Tasks**:
1. Add I/O wait monitoring (detect disk bottlenecks)
2. Add network connection tracking
3. Implement idle bot termination (free resources)
4. Add per-bot resource tracking (identify heavy bots)
5. Dashboard visualization of resource usage

### Phase 4: OS Integration (2-3 days)

**Tasks**:
1. Document cgroup setup
2. Add systemd service file with resource controls
3. Test on production-like environment
4. Performance benchmarks

---

## Expected Behavior (After Implementation)

```
Worker starts: max_bots=300, CPU threshold=80%

  0:00 - Spawn 50 bots/s, CPU=20%, Memory=30%
         → SpeedUp signal: increase target to 300
  
  0:30 - 150 bots active, CPU=55%, Memory=60%
         → Normal operation
  
  1:00 - 250 bots active, CPU=82%, Memory=78%
         → SlowDown signal: reduce spawn rate, target=225
  
  1:15 - 225 bots active, CPU=75%, Memory=72%
         → Stable (within thresholds)
  
  1:30 - External load spike (validator sync), CPU=92%
         → EmergencyStop: pause spawning, terminate 20 idle bots
         → New target=205
  
  2:00 - External load complete, CPU=65%, Memory=68%
         → Resume: restore target to 225 (not full 300, gradual)
  
  2:30 - CPU=60%, Memory=65%
         → SpeedUp: increase target to 250
```

**Result**: Operates near maximum capacity without impacting other services.

---

## Metrics to Add

```rust
// Prometheus metrics
testbots_worker_cpu_percent
testbots_worker_memory_percent
testbots_worker_target_bots    // Dynamic target
testbots_worker_throttle_events{type="slowdown|stop|resume"}
testbots_bot_spawn_rate        // Actual spawn rate
```

---

## Alternative: Manual Resource Control (Interim)

Until dynamic management is implemented, use these approaches:

### 1. Static Tuning
```bash
# Measure baseline resource usage
top -b -n 1 | grep "Cpu(s)"

# If system has 32 cores, 64GB RAM, and runs validator:
# Reserve: 8 cores (25%), 16GB (25%) for validator
# Available: 24 cores, 48GB for bots
# Each bot uses ~0.1 core, ~50MB RAM
# Safe limit: 240 bots (24 cores / 0.1) or 960 bots (48GB / 50MB)
# Choose conservative: 240 bots

acdc-botnet worker start --max-bots 240
```

### 2. Monitoring + Manual Adjustment
```bash
# Monitor in separate terminal
watch -n 5 'top -b -n 1 | head -20'

# If CPU >80%, reduce bots:
# (Requires restart, no hot-reload yet)
pkill acdc-botnet
acdc-botnet worker start --max-bots 180
```

### 3. Cgroups (Immediate Protection)
```bash
# Limit worker to 50% CPU, 16GB RAM
sudo cgcreate -g cpu,memory:acdc-botnet
sudo cgset -r cpu.shares=512 acdc-botnet
sudo cgset -r memory.limit_in_bytes=16G acdc-botnet
sudo cgexec -g cpu,memory:acdc-botnet \
  acdc-botnet worker start --max-bots 300
```

---

## Recommendation

**For production deployment**:
1. **Immediate**: Use cgroups + conservative max-bots (50% of capacity)
2. **Short-term** (1-2 weeks): Implement Phase 1 + 2 (monitoring + throttling)
3. **Long-term**: Full dynamic resource management + OS integration

This ensures the system is "good neighbor" and doesn't monopolize resources.
