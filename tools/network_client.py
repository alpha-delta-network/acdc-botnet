"""
network_client.py — REST client for AlphaOS / DeltaOS testnet nodes.

Covers all endpoints used by T005 behavior implementations.
Designed to be sync-friendly (uses requests) for simplicity in the runner.
"""
import json
import time
import urllib.request
import urllib.error
import urllib.parse
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple


# ─── Response wrapper ────────────────────────────────────────────────────────

@dataclass
class Response:
    status: int
    body: Any          # parsed JSON or raw str
    raw: bytes = field(repr=False, default=b"")
    error: Optional[str] = None

    @property
    def ok(self) -> bool:
        return 200 <= self.status < 300

    def json_field(self, *keys, default=None):
        """Safely navigate nested JSON fields."""
        node = self.body
        for k in keys:
            if isinstance(node, dict):
                node = node.get(k)
            elif isinstance(node, list) and isinstance(k, int):
                node = node[k] if k < len(node) else None
            else:
                return default
            if node is None:
                return default
        return node if node is not None else default


# ─── HTTP helpers ────────────────────────────────────────────────────────────

def _do_request(
    method: str,
    url: str,
    body: Optional[bytes] = None,
    headers: Optional[Dict[str, str]] = None,
    timeout: int = 10,
) -> Response:
    headers = headers or {}
    if body and "Content-Type" not in headers:
        headers["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
            try:
                parsed = json.loads(raw)
            except (json.JSONDecodeError, ValueError):
                parsed = raw.decode("utf-8", errors="replace")
            return Response(status=resp.status, body=parsed, raw=raw)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            parsed = json.loads(raw)
        except Exception:
            parsed = raw.decode("utf-8", errors="replace")
        return Response(status=e.code, body=parsed, raw=raw, error=str(e))
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        return Response(status=0, body=None, error=str(e))


def _get(url: str, timeout: int = 10) -> Response:
    return _do_request("GET", url, timeout=timeout)


def _post(url: str, payload: Any, timeout: int = 30) -> Response:
    body = json.dumps(payload).encode()
    return _do_request("POST", url, body=body, timeout=timeout)


def _post_raw(url: str, data: bytes, content_type: str = "application/octet-stream", timeout: int = 30) -> Response:
    headers = {"Content-Type": content_type}
    return _do_request("POST", url, body=data, headers=headers, timeout=timeout)


# ─── AlphaOS client ──────────────────────────────────────────────────────────

class AlphaClient:
    """
    REST client for AlphaOS (port 3030) and health endpoint (port 3000).
    All methods return Response objects so callers can inspect status + body.
    """

    def __init__(self, host: str, rpc_port: int = 3030, health_port: int = 3000, timeout: int = 10):
        self.host = host
        self.rpc_base = f"http://{host}:{rpc_port}"
        self.health_base = f"http://{host}:{health_port}"
        self.timeout = timeout

    # ── Health ────────────────────────────────────────────────────────────────

    def health(self) -> Response:
        return _get(f"{self.health_base}/health", timeout=self.timeout)

    # ── Blocks ────────────────────────────────────────────────────────────────

    def get_height(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/block/height/latest", timeout=self.timeout)

    def get_block(self, height: int) -> Response:
        return _get(f"{self.rpc_base}/testnet/block/{height}", timeout=self.timeout)

    def get_latest_block(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/block/latest", timeout=self.timeout)

    # ── Transactions ──────────────────────────────────────────────────────────

    def broadcast_transaction(self, tx_json: str) -> Response:
        """Broadcast a serialized transaction (hex string or JSON-encoded)."""
        payload = {"transaction": tx_json}
        return _post(f"{self.rpc_base}/testnet/transaction/broadcast", payload, timeout=30)

    def broadcast_transaction_bytes(self, tx_bytes: bytes) -> Response:
        return _post_raw(f"{self.rpc_base}/testnet/transaction/broadcast", tx_bytes, timeout=30)

    def get_transaction(self, tx_id: str) -> Response:
        return _get(f"{self.rpc_base}/testnet/transaction/{tx_id}", timeout=self.timeout)

    def get_transaction_status(self, tx_id: str) -> Response:
        return _get(f"{self.rpc_base}/testnet/transaction/{tx_id}/status", timeout=self.timeout)

    def get_mempool(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/memoryPool/transactions", timeout=self.timeout)

    def get_mempool_size(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/memoryPool/size", timeout=self.timeout)

    # ── Accounts ──────────────────────────────────────────────────────────────

    def get_balance(self, address: str) -> Response:
        return _get(f"{self.rpc_base}/testnet/program/credits.aleo/mapping/account/{address}", timeout=self.timeout)

    def get_record(self, record_id: str) -> Response:
        return _get(f"{self.rpc_base}/testnet/record/{record_id}", timeout=self.timeout)

    # ── Governance ────────────────────────────────────────────────────────────

    def get_governance_proposals(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/program/governance.aleo/mapping/proposals", timeout=self.timeout)

    def get_governance_votes(self, proposal_id: str) -> Response:
        return _get(f"{self.rpc_base}/testnet/program/governance.aleo/mapping/votes/{proposal_id}", timeout=self.timeout)

    def get_governance_state(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/program/governance.aleo/mapping/state", timeout=self.timeout)

    # ── Network ───────────────────────────────────────────────────────────────

    def get_peers_count(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/peers/count", timeout=self.timeout)

    def get_peers(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/peers/all", timeout=self.timeout)

    def get_committee(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/latest/committee", timeout=self.timeout)

    def get_state_root(self) -> Response:
        return _get(f"{self.rpc_base}/testnet/stateRoot/latest", timeout=self.timeout)

    # ── Programs ──────────────────────────────────────────────────────────────

    def get_program(self, program_id: str) -> Response:
        return _get(f"{self.rpc_base}/testnet/program/{program_id}", timeout=self.timeout)

    def get_mapping_value(self, program_id: str, mapping: str, key: str) -> Response:
        return _get(
            f"{self.rpc_base}/testnet/program/{program_id}/mapping/{mapping}/{key}",
            timeout=self.timeout,
        )

    # ── Convenience ───────────────────────────────────────────────────────────

    def wait_for_confirmation(self, tx_id: str, timeout_sec: int = 60) -> Tuple[bool, Optional[str]]:
        """Poll until tx is confirmed or timeout. Returns (confirmed, status_str)."""
        deadline = time.time() + timeout_sec
        while time.time() < deadline:
            resp = self.get_transaction_status(tx_id)
            if resp.ok:
                status = resp.body if isinstance(resp.body, str) else resp.json_field("status")
                if status in ("confirmed", "finalized", "included"):
                    return True, status
                if status in ("rejected", "failed", "invalid"):
                    return False, status
            # Also check by getting the transaction itself
            resp2 = self.get_transaction(tx_id)
            if resp2.ok and resp2.body:
                return True, "confirmed"
            time.sleep(3)
        return False, "timeout"

    def get_height_int(self) -> Optional[int]:
        """Return current block height as int, or None on failure."""
        resp = self.get_height()
        if not resp.ok:
            return None
        body = resp.body
        if isinstance(body, int):
            return body
        if isinstance(body, dict):
            for k in ("height", "block_height", "latest"):
                if k in body:
                    try:
                        return int(body[k])
                    except (ValueError, TypeError):
                        pass
        try:
            return int(body)
        except (ValueError, TypeError):
            return None


# ─── DeltaOS client ──────────────────────────────────────────────────────────

class DeltaClient:
    """
    REST client for DeltaOS (port 3031).
    DEX, perpetuals, oracle endpoints.
    """

    def __init__(self, host: str, rpc_port: int = 3031, timeout: int = 10):
        self.host = host
        self.rpc_base = f"http://{host}:{rpc_port}"
        self.timeout = timeout

    def get_dex_pairs(self) -> Response:
        return _get(f"{self.rpc_base}/delta/dex/pairs", timeout=self.timeout)

    def get_orderbook(self, pair: str) -> Response:
        return _get(f"{self.rpc_base}/delta/dex/orderbook/{pair}", timeout=self.timeout)

    def submit_order(self, order_tx: dict) -> Response:
        return _post(f"{self.rpc_base}/delta/dex/order", order_tx, timeout=30)

    def get_oracle_price(self, asset: str) -> Response:
        return _get(f"{self.rpc_base}/delta/oracle/price/{asset}", timeout=self.timeout)

    def get_balance(self, address: str) -> Response:
        return _get(f"{self.rpc_base}/delta/account/{address}/balance", timeout=self.timeout)


# ─── Multi-node client ───────────────────────────────────────────────────────

class MultiNodeClient:
    """
    Wraps multiple AlphaClients for parallel checks across all 5 validators.
    """
    VALIDATOR_HOSTS = [
        "testnet001.ac-dc.network",
        "testnet002.ac-dc.network",
        "testnet003.ac-dc.network",
        "testnet004.ac-dc.network",
        "testnet005.ac-dc.network",
    ]
    PROVER_HOST = "testnet006.ac-dc.network"

    def __init__(self, hosts: Optional[List[str]] = None):
        self.hosts = hosts or self.VALIDATOR_HOSTS
        self.clients = {h: AlphaClient(h) for h in self.hosts}

    def get_all_heights(self) -> Dict[str, Optional[int]]:
        return {h: c.get_height_int() for h, c in self.clients.items()}

    def all_advancing(self, window_sec: int = 30) -> bool:
        """Check that all nodes advance height over a window."""
        h1 = self.get_all_heights()
        time.sleep(window_sec)
        h2 = self.get_all_heights()
        return all(
            h2.get(h) is not None and h1.get(h) is not None and h2[h] > h1[h]
            for h in self.hosts
        )

    def heights_agree(self, max_diff: int = 1) -> Tuple[bool, Dict]:
        """Check that all nodes are within max_diff blocks of each other."""
        heights = self.get_all_heights()
        valid = {h: v for h, v in heights.items() if v is not None}
        if not valid:
            return False, heights
        diff = max(valid.values()) - min(valid.values())
        return diff <= max_diff, heights
