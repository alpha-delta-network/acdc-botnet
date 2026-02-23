#!/bin/bash
# ACDC Botnet - Systemd Service Installation Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}Error: This script must be run as root${NC}"
   echo "Usage: sudo ./install.sh"
   exit 1
fi

echo -e "${GREEN}ACDC Botnet - Systemd Service Installation${NC}"
echo "=============================================="
echo ""

# 1. Install binary (if not already present)
if [[ ! -f /usr/local/bin/acdc-botnet ]]; then
    echo -e "${YELLOW}Warning: /usr/local/bin/acdc-botnet not found${NC}"
    echo "Please build and install the binary first:"
    echo "  cd /home/devops/working-repos/acdc-botnet"
    echo "  cargo build --release"
    echo "  sudo cp target/release/acdc-botnet /usr/local/bin/"
    echo "  sudo chmod +x /usr/local/bin/acdc-botnet"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# 2. Create configuration directory
echo -e "${GREEN}Creating configuration directory...${NC}"
mkdir -p /etc/acdc-botnet
mkdir -p /var/lib/acdc-botnet/checkpoints
mkdir -p /opt/acdc-botnet

# Set proper ownership (assuming 'devops' user exists)
if id "devops" &>/dev/null; then
    chown -R devops:devops /var/lib/acdc-botnet
    chown -R devops:devops /opt/acdc-botnet
    echo "  ✓ Set ownership to devops:devops"
else
    echo -e "${YELLOW}  Warning: 'devops' user not found, using root ownership${NC}"
fi

# 3. Copy service files
echo -e "${GREEN}Installing systemd service files...${NC}"
cp acdc-botnet-coordinator.service /etc/systemd/system/
cp acdc-botnet-worker@.service /etc/systemd/system/
echo "  ✓ Coordinator service installed"
echo "  ✓ Worker template service installed"

# 4. Create example worker configurations
echo -e "${GREEN}Creating example worker configurations...${NC}"

# Worker 1 (default configuration)
cat > /etc/acdc-botnet/worker-1.conf <<EOF
# Worker 1 - Default Configuration
COORDINATOR_ADDR=localhost:50051
WORKER_ID=worker-1
MAX_BOTS=300
CAPABILITIES=trader,user,governor
CPU_QUOTA=80%
MEMORY_MAX=16G
EOF

# Worker 2 (lighter configuration for co-located validator)
cat > /etc/acdc-botnet/worker-2.conf <<EOF
# Worker 2 - Light Configuration (for nodes running validators)
COORDINATOR_ADDR=localhost:50051
WORKER_ID=worker-2
MAX_BOTS=150
CAPABILITIES=trader,user
CPU_QUOTA=40%
MEMORY_MAX=8G
EOF

echo "  ✓ Created /etc/acdc-botnet/worker-1.conf"
echo "  ✓ Created /etc/acdc-botnet/worker-2.conf"

# 5. Reload systemd daemon
echo -e "${GREEN}Reloading systemd daemon...${NC}"
systemctl daemon-reload
echo "  ✓ Systemd daemon reloaded"

echo ""
echo -e "${GREEN}Installation complete!${NC}"
echo ""
echo "Next steps:"
echo ""
echo "1. Start the coordinator:"
echo "   sudo systemctl start acdc-botnet-coordinator"
echo "   sudo systemctl enable acdc-botnet-coordinator"
echo ""
echo "2. Start worker instance(s):"
echo "   sudo systemctl start acdc-botnet-worker@1"
echo "   sudo systemctl enable acdc-botnet-worker@1"
echo ""
echo "   (Optional) Start additional workers:"
echo "   sudo systemctl start acdc-botnet-worker@2"
echo "   sudo systemctl enable acdc-botnet-worker@2"
echo ""
echo "3. Check status:"
echo "   sudo systemctl status acdc-botnet-coordinator"
echo "   sudo systemctl status acdc-botnet-worker@1"
echo ""
echo "4. View logs:"
echo "   sudo journalctl -u acdc-botnet-coordinator -f"
echo "   sudo journalctl -u acdc-botnet-worker@1 -f"
echo ""
echo "5. Monitor resource usage:"
echo "   systemd-cgtop"
echo ""
echo "Configuration files:"
echo "  - Coordinator: /etc/systemd/system/acdc-botnet-coordinator.service"
echo "  - Worker template: /etc/systemd/system/acdc-botnet-worker@.service"
echo "  - Worker configs: /etc/acdc-botnet/worker-*.conf"
echo ""
echo "Data directories:"
echo "  - Checkpoints: /var/lib/acdc-botnet/checkpoints"
echo "  - Working dir: /opt/acdc-botnet"
echo ""
