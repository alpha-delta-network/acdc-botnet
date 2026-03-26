"""
network_client.py — REST client for adnet public API (port 8080).

All external-facing endpoints go through /api/v1/... on port 8080.
Ports 3030/3031 are internal-only (alphavm/deltavm direct).
"""
import json
import os
import time
import urllib.request
import urllib.error
import urllib.parse
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

# API key for adnet public API — any ak_/pk_ prefix is accepted as Standard tier.
# Set ADNET_API_KEY env var to override.
_DEFAULT_API_KEY = "ak_botnet_testnet_2026"


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
    # Attach API key if hitting port 8080
    if ":8080" in url and "Authorization" not in headers:
        api_key = os.environ.get("ADNET_API_KEY", _DEFAULT_API_KEY)
        headers["Authorization"] = f"Bearer {api_key}"

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


# ─── AlphaClient ─────────────────────────────────────────────────────────────

class AlphaClient:
    """
    REST client for adnet public API on port 8080.
    All routes use /api/v1/... prefix.
    Ports 3030/3031 are internal only — never used externally.
    """

    def __init__(self, host: str, port: int = 8080, timeout: int = 10):
        self.host = host
        self.base = f"http://{host}:{port}"
        self.timeout = timeout

    # ── Health ────────────────────────────────────────────────────────────────

    def health(self) -> Response:
        return _get(f"{self.base}/health", timeout=self.timeout)

    # ── Blocks ────────────────────────────────────────────────────────────────

    def get_height(self) -> Response:
        return _get(f"{self.base}/api/v1/chain/height", timeout=self.timeout)

    def get_block(self, height: int) -> Response:
        return _get(f"{self.base}/api/v1/blocks/{height}", timeout=self.timeout)

    def get_latest_block(self) -> Response:
        return _get(f"{self.base}/api/v1/blocks/latest", timeout=self.timeout)

    # ── Transactions ──────────────────────────────────────────────────────────

    def broadcast_transaction(self, tx_json) -> Response:
        """Submit a public transaction via adnet API."""
        payload = json.loads(tx_json) if isinstance(tx_json, str) else tx_json
        return _post(f"{self.base}/api/v1/transactions/submit/public", payload, timeout=30)

    def broadcast_transaction_bytes(self, tx_bytes: bytes) -> Response:
        return _post_raw(f"{self.base}/api/v1/transactions/submit/public", tx_bytes, timeout=30)

    def get_transaction(self, tx_id: str) -> Response:
        return _get(f"{self.base}/api/v1/blocks/by-tx/{tx_id}", timeout=self.timeout)

    def get_mempool(self) -> Response:
        return _get(f"{self.base}/api/v1/mempool", timeout=self.timeout)

    def get_mempool_size(self) -> Response:
        resp = self.get_mempool()
        if resp.ok and isinstance(resp.body, dict):
            # Return a synthetic size response
            size = resp.body.get("size", resp.body.get("count", 0))
            return Response(status=200, body=size)
        return resp

    # ── Accounts ──────────────────────────────────────────────────────────────

    def get_balance(self, address: str) -> Response:
        # Public API balance endpoint expects 128-char hex Grumpkin (Delta) addresses.
        # For Alpha bech32 addresses (ac1...), use state/path lookup instead.
        if address.startswith("ac1") or address.startswith("av1"):
            key_enc = urllib.parse.quote(f"credits.alpha/account/{address}", safe="")
            return _get(f"{self.base}/api/v1/state/path/{key_enc}", timeout=self.timeout)
        return _get(f"{self.base}/api/v1/addresses/{address}/balance", timeout=self.timeout)

    # ── Governance ────────────────────────────────────────────────────────────

    def get_governance_proposals(self) -> Response:
        return _get(f"{self.base}/api/v1/governance/proposals", timeout=self.timeout)

    def get_governance_proposal(self, proposal_id) -> Response:
        return _get(f"{self.base}/api/v1/governance/proposals/{proposal_id}", timeout=self.timeout)

    def get_governance_state(self) -> Response:
        return self.get_governance_proposals()

    def get_governance_votes(self, proposal_id) -> Response:
        return _get(f"{self.base}/api/v1/governance/proposals/{proposal_id}", timeout=self.timeout)

    # ── Network / Validators ──────────────────────────────────────────────────

    def get_committee(self) -> Response:
        return _get(f"{self.base}/api/v1/committee", timeout=self.timeout)

    def get_validators(self) -> Response:
        return _get(f"{self.base}/api/v1/validators", timeout=self.timeout)

    def get_state_root(self) -> Response:
        return _get(f"{self.base}/api/v1/state/root", timeout=self.timeout)

    def get_peers_count(self) -> Response:
        # No direct peers endpoint on public API — use validators as proxy
        return self.get_validators()

    def get_peers(self) -> Response:
        return self.get_validators()

    # ── Bridge ────────────────────────────────────────────────────────────────

    def get_bridge_state(self) -> Response:
        return _get(f"{self.base}/api/v1/bridge/state", timeout=self.timeout)

    def lock_for_bridge(self, payload: dict) -> Response:
        return _post(f"{self.base}/api/v1/bridge/lock", payload, timeout=30)

    # ── GCI governance ────────────────────────────────────────────────────────

    def get_gci_active(self) -> Response:
        return _get(f"{self.base}/api/governance/gci/active", timeout=self.timeout)

    def get_gci_roster(self, gid: str) -> Response:
        return _get(f"{self.base}/api/governance/gci/{gid}/roster", timeout=self.timeout)

    def register_gci(self, payload: dict) -> Response:
        return _post(f"{self.base}/api/governance/gci/register", payload, timeout=30)

    # ── Convenience ───────────────────────────────────────────────────────────

    def wait_for_confirmation(self, tx_id: str, timeout_sec: int = 60) -> Tuple[bool, Optional[str]]:
        """Poll until tx appears in a block or timeout."""
        deadline = time.time() + timeout_sec
        while time.time() < deadline:
            resp = self.get_transaction(tx_id)
            if resp.ok and resp.body:
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
            for k in ("alpha_height", "height", "block_height", "latest", "chain_height"):
                if k in body:
                    try:
                        return int(body[k])
                    except (ValueError, TypeError):
                        pass
        try:
            return int(body)
        except (ValueError, TypeError):
            return None


# ─── DeltaClient ─────────────────────────────────────────────────────────────

class DeltaClient:
    """
    REST client for Delta chain via adnet public API (port 8080).
    """

    def __init__(self, host: str, port: int = 8080, timeout: int = 10):
        self.host = host
        self.base = f"http://{host}:{port}"
        self.timeout = timeout

    def get_dex_pairs(self) -> Response:
        return _get(f"{self.base}/api/v1/dex/pairs", timeout=self.timeout)

    def get_orderbook(self, pair: str) -> Response:
        return _get(f"{self.base}/api/v1/dex/orderbook/{pair}", timeout=self.timeout)

    def submit_order(self, order_tx: dict) -> Response:
        payload = {
            "chain_id": "delta",
            "tx_bytes": json.dumps(order_tx).encode().hex(),
            "proof": "00" * 32,
            "fee": order_tx.get("fee", 150),
        }
        return _post(f"{self.base}/api/v1/transactions/submit/public", payload, timeout=30)

    def get_oracle_price(self, asset: str) -> Response:
        return _get(f"{self.base}/api/v1/oracle/prices", timeout=self.timeout)

    def get_balance(self, address: str) -> Response:
        return _get(f"{self.base}/api/v1/addresses/{address}/balance", timeout=self.timeout)

    def get_bridge_state(self) -> Response:
        return _get(f"{self.base}/api/v1/bridge/state", timeout=self.timeout)

    def get_delta_state_root(self) -> Response:
        return _get(f"{self.base}/api/v1/state/root/delta", timeout=self.timeout)


# ─── Multi-node client ───────────────────────────────────────────────────────

class MultiNodeClient:
    """
    Wraps multiple AlphaClients for parallel checks across all validators.
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
