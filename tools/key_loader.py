"""
key_loader.py — Load testnet key YAML and expose typed key sets.

Expected file format (written by testnet-gen-keys.sh):
  deploy_id: <id>
  generated_at: <iso8601>
  governor_keys:
    - index: 0
      alpha_addr: ac1...
      private_key: ap1...
      view_key: av1...
  validator_keys:
    - index: 0
      alpha_addr: ac1...
      private_key: ap1...
  funded_wallets:
    - index: 0
      alpha_addr: ac1...
      private_key: ap1...
      view_key: av1...
      ax_balance: 1000000
      dx_balance: 500000
  prover_key:
    alpha_addr: ac1...
    private_key: ap1...
"""
from __future__ import annotations

import os
import re
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

try:
    import yaml
except ImportError:
    import json as yaml  # type: ignore  # fallback: only load JSON-as-YAML

# ─── Key entry dataclass ──────────────────────────────────────────────────────

@dataclass
class KeyEntry:
    alpha_addr: str
    private_key: str
    view_key: Optional[str] = None
    ax_balance: int = 0
    dx_balance: int = 0
    index: int = 0

    def to_dict(self) -> Dict[str, Any]:
        d: Dict[str, Any] = {
            "alpha_addr": self.alpha_addr,
            "private_key": self.private_key,
            "index": self.index,
        }
        if self.view_key:
            d["view_key"] = self.view_key
        if self.ax_balance:
            d["ax_balance"] = self.ax_balance
        if self.dx_balance:
            d["dx_balance"] = self.dx_balance
        return d


# ─── KeySet ───────────────────────────────────────────────────────────────────

@dataclass
class KeySet:
    """Typed representation of a testnet key YAML file."""
    deploy_id: str
    generated_at: str
    governor_keys: List[KeyEntry] = field(default_factory=list)
    validator_keys: List[KeyEntry] = field(default_factory=list)
    funded_wallets: List[KeyEntry] = field(default_factory=list)
    prover_key: Optional[KeyEntry] = None

    # ── Reference resolution ──────────────────────────────────────────────────

    def resolve(self, ref: str) -> Any:
        """
        Resolve a key reference string used in scenario YAML.

        Examples:
          "keys.funded_wallets[0]"            → KeyEntry
          "keys.funded_wallets[0].alpha_addr" → str
          "keys.governor_keys[2].private_key" → str
          "keys.prover_key.alpha_addr"        → str
        """
        ref = ref.strip()
        if not ref.startswith("keys."):
            return None
        tail = ref[len("keys."):]
        return self._walk(tail)

    def _walk(self, path: str) -> Any:
        parts = re.split(r"[\.\[]", path)
        node: Any = {
            "governor_keys": [k.to_dict() for k in self.governor_keys],
            "validator_keys": [k.to_dict() for k in self.validator_keys],
            "funded_wallets": [k.to_dict() for k in self.funded_wallets],
            "prover_key": self.prover_key.to_dict() if self.prover_key else {},
        }
        for part in parts:
            part = part.rstrip("]")
            if part == "":
                continue
            if isinstance(node, dict):
                node = node.get(part)
            elif isinstance(node, list):
                try:
                    node = node[int(part)]
                except (ValueError, IndexError):
                    return None
            else:
                return None
            if node is None:
                return None
        return node

    def wallet_for_role(self, role: str, index: int = 0) -> Optional[KeyEntry]:
        """Return a key entry for a given role name."""
        role_map = {
            "governor": self.governor_keys,
            "validator": self.validator_keys,
            "funded_wallet": self.funded_wallets,
            "general_user": self.funded_wallets,
            "attacker": self.funded_wallets,
            "prover": [self.prover_key] if self.prover_key else [],
        }
        entries = role_map.get(role, self.funded_wallets)
        if not entries:
            return None
        return entries[index % len(entries)]

    def all_wallets(self) -> List[KeyEntry]:
        return list(self.funded_wallets)

    def governor(self, index: int = 0) -> Optional[KeyEntry]:
        if not self.governor_keys:
            return None
        return self.governor_keys[index % len(self.governor_keys)]

    def validator(self, index: int = 0) -> Optional[KeyEntry]:
        if not self.validator_keys:
            return None
        return self.validator_keys[index % len(self.validator_keys)]


# ─── Loader ───────────────────────────────────────────────────────────────────

def _parse_entry(raw: Dict[str, Any]) -> KeyEntry:
    return KeyEntry(
        alpha_addr=raw.get("alpha_addr", ""),
        private_key=raw.get("private_key", ""),
        view_key=raw.get("view_key"),
        ax_balance=int(raw.get("ax_balance", 0)),
        dx_balance=int(raw.get("dx_balance", 0)),
        index=int(raw.get("index", 0)),
    )


def load(path: str) -> KeySet:
    """Load a testnet key YAML file and return a KeySet."""
    if not os.path.exists(path):
        raise FileNotFoundError(f"Key file not found: {path}")

    with open(path) as f:
        raw = yaml.safe_load(f)

    if not isinstance(raw, dict):
        raise ValueError(f"Invalid key file format: {path}")

    ks = KeySet(
        deploy_id=str(raw.get("deploy_id", "unknown")),
        generated_at=str(raw.get("generated_at", "")),
    )

    for entry in raw.get("governor_keys", []):
        ks.governor_keys.append(_parse_entry(entry))

    for entry in raw.get("validator_keys", []):
        ks.validator_keys.append(_parse_entry(entry))

    for entry in raw.get("funded_wallets", []):
        ks.funded_wallets.append(_parse_entry(entry))

    pk = raw.get("prover_key")
    if pk and isinstance(pk, dict):
        ks.prover_key = _parse_entry(pk)

    return ks


def make_stub(deploy_id: str = "test-000") -> KeySet:
    """Return a stub KeySet for testing without a real key file."""
    def _mk(prefix: str, i: int) -> KeyEntry:
        return KeyEntry(
            alpha_addr=f"ac1{prefix}{i:04d}00000000000000000000000000000000000000000000",
            private_key=f"ap1{prefix}{i:04d}00000000000000000000000000000000000000000000",
            view_key=f"av1{prefix}{i:04d}00000000000000000000000000000000000000000000",
            ax_balance=1_000_000,
            dx_balance=500_000,
            index=i,
        )

    ks = KeySet(deploy_id=deploy_id, generated_at="2026-03-15T00:00:00Z")
    ks.governor_keys = [_mk("gov", i) for i in range(5)]
    ks.validator_keys = [_mk("val", i) for i in range(5)]
    ks.funded_wallets = [_mk("usr", i) for i in range(20)]
    ks.prover_key = _mk("prv", 0)
    return ks
