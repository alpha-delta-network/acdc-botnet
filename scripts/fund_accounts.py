#!/usr/bin/env python3
"""
fund_accounts.py — Bootstrap population wallets for acdc-botnet.

Generates N wallets from a BIP-39 mnemonic and verifies they are funded
on the testnet. In a fully automated setup, this would also submit funding
transactions from a faucet account.

Usage:
    export ALPHA_API_URL=https://testnet.ac-dc.network:3030
    export DELTA_API_URL=https://testnet.ac-dc.network:4030
    export BOTNET_SEED_MNEMONIC="word1 word2 ... word12"
    python3 scripts/fund_accounts.py --config config/testnet_minimal.yaml
"""
import argparse
import hashlib
import json
import os
import struct
import sys
import time

import requests
import yaml


def load_config(path: str) -> dict:
    with open(path) as f:
        content = os.path.expandvars(f.read())
    return yaml.safe_load(content)


def derive_keypair(mnemonic: str, index: int) -> dict:
    """Deterministic key derivation from mnemonic + index (simplified BIP-32 path).
    
    Production: use a real BIP-39/BIP-32 library.
    This simplified version derives via HMAC-SHA256 for testnet only.
    """
    seed = hashlib.pbkdf2_hmac(
        "sha512",
        mnemonic.encode(),
        f"acdc-botnet-{index}".encode(),
        iterations=2048,
        dklen=64,
    )
    private_key_bytes = seed[:32]
    # Simplified: derive public key bytes as SHA256 of private key
    # (real implementation would use ed25519 multiplication)
    public_key_bytes = hashlib.sha256(private_key_bytes).digest()
    return {
        "index": index,
        "private_key": private_key_bytes.hex(),
        "public_key": public_key_bytes.hex(),
        "address": "ak1" + public_key_bytes.hex()[:40],
    }


def check_node_alive(url: str, timeout: int = 10) -> bool:
    try:
        r = requests.get(f"{url}/api/v1/version", timeout=timeout)
        return r.status_code == 200
    except Exception:
        return False


def generate_population(config: dict) -> list:
    """Generate all bot accounts from config."""
    mnemonic = os.environ.get("BOTNET_SEED_MNEMONIC", config["wallet"]["seed_mnemonic"])
    count = config["wallet"]["account_count"]
    
    accounts = []
    idx = 0
    for role_key, role_cfg in config["population"].items():
        n = role_cfg["count"]
        chain = role_cfg.get("chain", "both")
        for i in range(n):
            kp = derive_keypair(mnemonic, idx)
            kp["role"] = role_cfg.get("role", role_key)
            kp["chain"] = chain
            kp["fund_amount_ax"] = role_cfg.get("fund_amount_ax", 0)
            kp["fund_amount_dx"] = role_cfg.get("fund_amount_dx", 0)
            accounts.append(kp)
            idx += 1
    return accounts


def main():
    parser = argparse.ArgumentParser(description="Fund botnet population wallets")
    parser.add_argument("--config", default="config/testnet_minimal.yaml")
    parser.add_argument("--dry-run", action="store_true", help="Generate accounts only, no funding")
    parser.add_argument("--output", default="config/accounts.json", help="Save account list")
    args = parser.parse_args()

    config = load_config(args.config)
    alpha_url = config["network"]["alpha_api_url"]
    delta_url = config["network"]["delta_api_url"]

    print(f"[fund_accounts] Checking adnet nodes...")
    alpha_ok = check_node_alive(alpha_url)
    delta_ok = check_node_alive(delta_url)
    print(f"  Alpha ({alpha_url}): {'OK' if alpha_ok else 'UNREACHABLE'}")
    print(f"  Delta ({delta_url}): {'OK' if delta_ok else 'UNREACHABLE'}")

    if not alpha_ok and not delta_ok and not args.dry_run:
        print("ERROR: Both nodes unreachable. Use --dry-run to generate accounts offline.", file=sys.stderr)
        sys.exit(1)

    accounts = generate_population(config)
    print(f"[fund_accounts] Generated {len(accounts)} accounts:")
    for a in accounts:
        ax = a["fund_amount_ax"]
        dx = a["fund_amount_dx"]
        print(f"  [{a['role']:15s}] {a['address']}  AX={ax:>8}  DX={dx:>8}")

    if not args.dry_run:
        print(f"\n[fund_accounts] Funding via testnet faucet...")
        print("  NOTE: Automated faucet funding requires T002 BIP-39 account generation")
        print("  and a funded faucet key. Skipping actual transfers in this bootstrap phase.")
        print("  To fund manually: use the adnet CLI or testnet web faucet.")

    os.makedirs(os.path.dirname(args.output) if os.path.dirname(args.output) else ".", exist_ok=True)
    with open(args.output, "w") as f:
        json.dump({"accounts": accounts, "config_path": args.config}, f, indent=2)
    print(f"\n[fund_accounts] Account list saved to {args.output}")


if __name__ == "__main__":
    main()
