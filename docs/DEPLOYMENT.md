# ACDC Botnet - Production Deployment Guide

## Overview

This guide covers deploying ACDC Botnet in production with proper resource management, systemd integration, and multi-server orchestration.

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Coordinator (Command & Control)                    │
│  - ci.ac-dc.network:50051                          │
│  - Scenario orchestration                           │
│  - Bot distribution                                 │
│  - Metrics aggregation                              │
│  - systemd service: acdc-botnet-coordinator         │
└──────────┬──────────────────────────────────────────┘
           │ gRPC bidirectional streams
           ├─────────────────┬─────────────────┬──────────────────
           │                 │                 │
     ┌─────▼─────┐     ┌─────▼─────┐     ┌─────▼─────┐
     │  Worker 1 │     │  Worker 2 │     │  Worker N │
     │  (GPU)    │     │  (CPU)    │     │  (CPU)    │
     ├───────────┤     ├───────────┤     ├───────────┤
     │ 50 bots   │     │ 200 bots  │     │ 200 bots  │
     │ - Provers │     │ - Traders │     │ - Users   │
     │ - Users   │     │ - Users   │     │ - Govs    │
     └───────────┘     └───────────┘     └───────────┘
```

**Key Components:**
- **Coordinator**: Single instance, lightweight (25% CPU, 2GB RAM)
- **Workers**: Multiple instances, configurable resources (40-80% CPU, 8-16GB RAM)
- **Protocol**: gRPC over TCP (port 50051)
- **Orchestration**: systemd with resource controls

---

## Prerequisites

### System Requirements

#### Coordinator Server
- **CPU**: 2-4 cores
- **RAM**: 4GB minimum
- **Network**: Public IP or accessible to worker nodes
- **OS**: Linux with systemd (Ubuntu 20.04+, RHEL 8+, Debian 11+)

#### Worker Servers
- **CPU**: 8+ cores recommended (4 cores minimum)
- **RAM**: 16GB recommended (8GB minimum)
- **Network**: Access to coordinator on port 50051
- **GPU**: Optional (for prover bots with ZK proof generation)
- **OS**: Linux with systemd

### Software Dependencies
- Rust 1.92.0+ (for building)
- systemd 240+ (for resource controls)
- gRPC libraries (included in binary)

---

## Installation

### Step 1: Build Binary

On build server or CI:

```bash
cd /home/devops/working-repos/acdc-botnet
cargo build --release --all-features

# Binary will be at: target/release/acdc-botnet
```

### Step 2: Deploy to Servers

#### Deploy to Coordinator

```bash
# Copy binary
scp target/release/acdc-botnet coordinator.example.com:/tmp/
ssh coordinator.example.com

# Install binary
sudo mv /tmp/acdc-botnet /usr/local/bin/
sudo chmod +x /usr/local/bin/acdc-botnet
```

#### Deploy to Workers

```bash
# Copy to all worker nodes
for host in worker1 worker2 worker3; do
    scp target/release/acdc-botnet $host:/tmp/
    ssh $host "sudo mv /tmp/acdc-botnet /usr/local/bin/ && sudo chmod +x /usr/local/bin/acdc-botnet"
done
```

### Step 3: Install Systemd Services

#### On Coordinator

```bash
# Copy systemd files
cd /home/devops/working-repos/acdc-botnet
scp systemd/* coordinator.example.com:/tmp/

# Install services
ssh coordinator.example.com
cd /tmp
sudo ./install.sh
```

#### On Each Worker

```bash
# Copy systemd files
scp systemd/* worker1.example.com:/tmp/

# Install services
ssh worker1.example.com
cd /tmp
sudo ./install.sh
```

---

## Configuration

### Coordinator Configuration

Edit `/etc/systemd/system/acdc-botnet-coordinator.service` if needed:

```ini
[Service]
# Default bind address (0.0.0.0 = all interfaces)
ExecStart=/usr/local/bin/acdc-botnet coordinator start \
  --bind 0.0.0.0:50051 \
  --checkpointing \
  --checkpoint-interval 30s \
  --checkpoint-dir /var/lib/acdc-botnet/checkpoints \
  --metrics-port 9090

# Resource limits (adjust based on coordinator load)
CPUQuota=25%
MemoryMax=2G
```

### Worker Configuration

Each worker has a configuration file at `/etc/acdc-botnet/worker-<N>.conf`.

**Example: High-capacity worker** (`/etc/acdc-botnet/worker-1.conf`):
```bash
# Coordinator address (REQUIRED - update with your coordinator IP/hostname)
COORDINATOR_ADDR=ci.ac-dc.network:50051

# Worker identification
WORKER_ID=worker-1

# Maximum bots (adjust based on server capacity)
MAX_BOTS=300

# Bot capabilities
CAPABILITIES=trader,user,governor

# Resource limits
CPU_QUOTA=80%
MEMORY_MAX=16G
```

**Example: Co-located with validator** (`/etc/acdc-botnet/worker-2.conf`):
```bash
# Lighter configuration for servers running validators
COORDINATOR_ADDR=ci.ac-dc.network:50051
WORKER_ID=worker-2
MAX_BOTS=150
CAPABILITIES=trader,user
CPU_QUOTA=40%
MEMORY_MAX=8G
```

**Example: GPU-enabled worker** (`/etc/acdc-botnet/worker-gpu.conf`):
```bash
# GPU worker for ZK proof generation
COORDINATOR_ADDR=ci.ac-dc.network:50051
WORKER_ID=worker-gpu
MAX_BOTS=50
CAPABILITIES=prover,trader,user
CPU_QUOTA=60%
MEMORY_MAX=12G
```

### Bot Capability Types

| Capability | Description | Resource Requirements |
|-----------|-------------|---------------------|
| `user` | General users (transfers, queries) | Low CPU, Low RAM |
| `trader` | DEX trading (spot, perpetuals) | Medium CPU, Medium RAM |
| `governor` | Governance voting | Low CPU, Low RAM |
| `prover` | ZK proof generation | High CPU, High RAM, GPU optional |
| `validator` | Consensus participation | High CPU, Medium RAM |
| `liquidity_provider` | DEX liquidity operations | Medium CPU, Medium RAM |

---

## Starting Services

### Start Coordinator (First)

```bash
# On coordinator server
sudo systemctl start acdc-botnet-coordinator
sudo systemctl enable acdc-botnet-coordinator  # Start on boot

# Check status
sudo systemctl status acdc-botnet-coordinator

# View logs
sudo journalctl -u acdc-botnet-coordinator -f
```

**Expected log output:**
```
Coordinator listening on 0.0.0.0:50051
Checkpointing enabled: interval=30s, dir=/var/lib/acdc-botnet/checkpoints
Metrics server started on port 9090
```

### Start Workers (After Coordinator)

```bash
# On each worker server
sudo systemctl start acdc-botnet-worker@1
sudo systemctl enable acdc-botnet-worker@1

# Check status
sudo systemctl status acdc-botnet-worker@1

# View logs
sudo journalctl -u acdc-botnet-worker@1 -f
```

**Expected log output:**
```
Connecting to coordinator at ci.ac-dc.network:50051
Connected successfully
Worker registered: worker-1, capacity=300 bots
Capabilities: trader, user, governor
Waiting for bot assignments...
```

### Start Multiple Worker Instances

```bash
# Start workers 1-3 on same host (if sufficient resources)
sudo systemctl start acdc-botnet-worker@1
sudo systemctl start acdc-botnet-worker@2
sudo systemctl start acdc-botnet-worker@3

# Enable on boot
sudo systemctl enable acdc-botnet-worker@{1,2,3}
```

---

## Running Scenarios

### From Coordinator

```bash
# SSH to coordinator
ssh coordinator.example.com

# Run scenario (coordinator distributes bots to workers)
acdc-botnet run daily-network-ops --duration 10m

# Run high-load scenario
acdc-botnet run peak-tps-stress --workers 5 --bots-per-worker 200

# Check status
acdc-botnet status --show-workers
```

**Output:**
```
Coordinator: ci.ac-dc.network:50051
Workers: 5 active, 0 down
  worker-1 (GPU): 50/50 bots, 15% CPU, 4GB RAM
  worker-2 (CPU): 200/200 bots, 80% CPU, 8GB RAM
  worker-3 (CPU): 200/200 bots, 82% CPU, 8GB RAM
Total: 1000 bots, 3500 TPS, 0.2% errors
```

---

## Monitoring

### Systemd Resource Monitoring

```bash
# Real-time resource usage for all services
systemd-cgtop

# Specific service metrics
systemctl show acdc-botnet-coordinator -p CPUUsageNSec -p MemoryCurrent
systemctl show acdc-botnet-worker@1 -p CPUUsageNSec -p MemoryCurrent
```

### Prometheus Metrics

Coordinator exposes metrics on port 9090:

```bash
# Query coordinator metrics
curl http://coordinator.example.com:9090/metrics

# Key metrics:
# - testbots_worker_count{status="active"}
# - testbots_worker_count{status="down"}
# - testbots_total_bots
# - testbots_global_tps
# - testbots_scenario_duration_seconds
```

### Log Aggregation

```bash
# View all coordinator logs
sudo journalctl -u acdc-botnet-coordinator --since "1 hour ago"

# View all worker logs
sudo journalctl -u 'acdc-botnet-worker@*' --since "1 hour ago"

# Follow logs from all services
sudo journalctl -u acdc-botnet-coordinator -u 'acdc-botnet-worker@*' -f
```

---

## Resource Tuning

### Adjusting CPU Quotas

```bash
# Edit worker configuration
sudo systemctl edit acdc-botnet-worker@1

# Add override:
[Service]
CPUQuota=60%

# Reload and restart
sudo systemctl daemon-reload
sudo systemctl restart acdc-botnet-worker@1
```

### Adjusting Memory Limits

```bash
# Edit worker configuration
sudo systemctl edit acdc-botnet-worker@1

# Add override:
[Service]
MemoryMax=12G

# Reload and restart
sudo systemctl daemon-reload
sudo systemctl restart acdc-botnet-worker@1
```

### Bot Capacity Tuning

Edit `/etc/acdc-botnet/worker-N.conf`:

```bash
# Rule of thumb:
# - Each bot: ~0.1 CPU core, ~50MB RAM
# - 8 cores, 16GB RAM → MAX_BOTS=150-200
# - 16 cores, 32GB RAM → MAX_BOTS=300-400
# - 32 cores, 64GB RAM → MAX_BOTS=600-800

MAX_BOTS=400

# Then restart worker
sudo systemctl restart acdc-botnet-worker@1
```

---

## Troubleshooting

### Worker Cannot Connect to Coordinator

**Symptom:**
```
Error: Failed to connect to coordinator at ci.ac-dc.network:50051
```

**Solutions:**
1. Check coordinator is running: `sudo systemctl status acdc-botnet-coordinator`
2. Verify firewall allows port 50051: `sudo ufw allow 50051/tcp`
3. Test connectivity: `telnet ci.ac-dc.network 50051`
4. Check coordinator logs: `sudo journalctl -u acdc-botnet-coordinator`

### Out-of-Memory Kills

**Symptom:**
```
systemd[1]: acdc-botnet-worker@1.service: A process of this unit has been killed by the OOM killer.
```

**Solutions:**
1. Reduce `MAX_BOTS` in `/etc/acdc-botnet/worker-N.conf`
2. Increase `MemoryMax` in systemd service (if physical RAM available)
3. Enable swap (last resort): `sudo swapon /swapfile`

### CPU Throttling

**Symptom:**
```
Worker performance degraded, TPS dropping
```

**Solutions:**
1. Check CPU usage: `systemd-cgtop`
2. Increase `CPUQuota` if underutilized
3. Reduce `MAX_BOTS` if overloaded
4. Check for other processes competing for CPU

### Coordinator Checkpointing Failures

**Symptom:**
```
Error: Failed to write checkpoint to /var/lib/acdc-botnet/checkpoints
```

**Solutions:**
1. Check directory permissions: `ls -ld /var/lib/acdc-botnet/checkpoints`
2. Ensure directory exists: `sudo mkdir -p /var/lib/acdc-botnet/checkpoints`
3. Set ownership: `sudo chown -R devops:devops /var/lib/acdc-botnet`
4. Check disk space: `df -h /var/lib/acdc-botnet`

---

## Scaling

### Adding Workers

1. Deploy binary to new server
2. Install systemd services
3. Configure worker: `/etc/acdc-botnet/worker-N.conf`
4. Start worker: `sudo systemctl start acdc-botnet-worker@N`
5. Worker auto-registers with coordinator

### Removing Workers

```bash
# Graceful shutdown (allows 60s for bots to finish)
sudo systemctl stop acdc-botnet-worker@N

# Disable on boot
sudo systemctl disable acdc-botnet-worker@N

# Coordinator will detect worker down after 3 missed heartbeats (15s)
# and migrate bots to healthy workers
```

### Horizontal Scaling Limits

- **Theoretical**: 100+ workers per coordinator
- **Tested**: 10 workers, 3000 total bots
- **Bottleneck**: Coordinator gRPC throughput (~10k messages/sec)

---

## Security Hardening

All services include security hardening:

```ini
# Systemd security directives
NoNewPrivileges=true        # Cannot escalate privileges
PrivateTmp=true             # Isolated /tmp directory
ProtectSystem=strict        # Read-only /usr, /boot, /efi
ProtectHome=true            # No access to /home
ReadWritePaths=/var/lib/acdc-botnet  # Only write to data dir
```

**Additional recommendations:**
1. Run coordinator behind reverse proxy (nginx/caddy) with TLS
2. Use firewall to restrict port 50051 to worker IPs only
3. Enable SELinux or AppArmor for additional confinement
4. Rotate checkpoint files periodically to prevent disk exhaustion

---

## Maintenance

### Service Restart (Zero Downtime)

```bash
# Restart workers one at a time (coordinator migrates bots)
sudo systemctl restart acdc-botnet-worker@1
# Wait 60s for bots to migrate
sudo systemctl restart acdc-botnet-worker@2
# Wait 60s
sudo systemctl restart acdc-botnet-worker@3
```

### Coordinator Restart (With Downtime)

```bash
# Coordinator restart causes brief outage (~5-10s)
sudo systemctl restart acdc-botnet-coordinator

# Workers will reconnect automatically
# Bots are recreated from last checkpoint (30s intervals)
```

### Log Rotation

Logs are managed by journald. Configure retention:

```bash
# Edit journald config
sudo nano /etc/systemd/journald.conf

# Set limits:
SystemMaxUse=1G
MaxRetentionSec=7day

# Restart journald
sudo systemctl restart systemd-journald
```

---

## Performance Benchmarks

| Configuration | Bots | TPS | CPU Usage | RAM Usage | Latency (p95) |
|--------------|------|-----|-----------|-----------|---------------|
| 1 worker, 8 cores | 200 | 1,500 | 70% | 10GB | 250ms |
| 3 workers, 24 cores | 600 | 4,500 | 65% | 28GB | 280ms |
| 5 workers, 40 cores | 1000 | 7,500 | 70% | 45GB | 320ms |

**Notes:**
- Measurements on testnet (Alpha/Delta dual-chain)
- Mixed workload (50% trades, 30% transfers, 20% governance)
- Network latency: <50ms coordinator↔workers

---

## Quick Reference

### Essential Commands

```bash
# Coordinator
sudo systemctl start acdc-botnet-coordinator
sudo systemctl status acdc-botnet-coordinator
sudo journalctl -u acdc-botnet-coordinator -f

# Workers
sudo systemctl start acdc-botnet-worker@1
sudo systemctl status acdc-botnet-worker@1
sudo journalctl -u acdc-botnet-worker@1 -f

# Resource monitoring
systemd-cgtop
systemctl show acdc-botnet-worker@1 -p CPUUsageNSec -p MemoryCurrent

# Run scenario
acdc-botnet run daily-network-ops
acdc-botnet status --show-workers

# Metrics
curl http://coordinator.example.com:9090/metrics
```

### Configuration Files

| File | Purpose |
|------|---------|
| `/etc/systemd/system/acdc-botnet-coordinator.service` | Coordinator service |
| `/etc/systemd/system/acdc-botnet-worker@.service` | Worker template service |
| `/etc/acdc-botnet/worker-N.conf` | Per-worker configuration |
| `/var/lib/acdc-botnet/checkpoints/` | Coordinator state checkpoints |
| `/opt/acdc-botnet/` | Working directory |

---

## Next Steps

1. **Dynamic Resource Management** (see `RESOURCE_MANAGEMENT.md`):
   - Implement ResourceMonitor for real-time CPU/memory tracking
   - Add ThrottleController for automatic bot scaling
   - Target: Operate at 80% capacity without manual tuning

2. **Enhanced Metrics**:
   - Add per-bot resource tracking
   - Implement anomaly detection (3-sigma + MAD)
   - Dashboard visualization (Grafana integration)

3. **High Availability**:
   - Multi-coordinator consensus (Raft/etcd)
   - Worker failover testing at scale (>10 workers)
   - Zero-downtime upgrades

---

For questions or issues, see:
- **Repository**: https://source.ac-dc.network/alpha-delta-network/acdc-botnet
- **Documentation**: `/docs/` directory
- **CI Status**: https://ci.ac-dc.network/alpha-delta-network/acdc-botnet
