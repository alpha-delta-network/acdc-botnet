"""
conftest.py — Shared pytest fixtures for botnet tool tests.
"""
import sys
from pathlib import Path

# Add tools/ to path so imports work in test files
TOOLS_DIR = Path(__file__).parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import pytest
from unittest.mock import MagicMock, patch
from key_loader import KeySet, KeyEntry, make_stub
from network_client import Response


@pytest.fixture
def stub_keys() -> KeySet:
    return make_stub("test-deploy-001")


@pytest.fixture
def sample_key_yaml(tmp_path) -> Path:
    p = tmp_path / "testnet-keys-test001.yaml"
    p.write_text("""
deploy_id: test001
generated_at: "2026-03-15T00:00:00Z"
governor_keys:
  - index: 0
    alpha_addr: ac1gov000000000000000000000000000000000000000000
    private_key: ap1gov000000000000000000000000000000000000000000
    view_key: av1gov000000000000000000000000000000000000000000
validator_keys:
  - index: 0
    alpha_addr: ac1val000000000000000000000000000000000000000000
    private_key: ap1val000000000000000000000000000000000000000000
funded_wallets:
  - index: 0
    alpha_addr: ac1usr000000000000000000000000000000000000000000
    private_key: ap1usr000000000000000000000000000000000000000000
    view_key: av1usr000000000000000000000000000000000000000000
    ax_balance: 1000000
    dx_balance: 500000
  - index: 1
    alpha_addr: ac1usr100000000000000000000000000000000000000000
    private_key: ap1usr100000000000000000000000000000000000000000
    ax_balance: 1000000
prover_key:
  index: 0
  alpha_addr: ac1prv000000000000000000000000000000000000000000
  private_key: ap1prv000000000000000000000000000000000000000000
""")
    return p


@pytest.fixture
def ok_response() -> Response:
    return Response(status=200, body={"transaction_id": "tx123"}, raw=b'{"transaction_id":"tx123"}')


@pytest.fixture
def rejection_response() -> Response:
    return Response(status=400, body={"error": "INVALID_PROOF"}, raw=b'{"error":"INVALID_PROOF"}',
                    error="HTTP Error 400")


@pytest.fixture
def height_response() -> Response:
    return Response(status=200, body=42, raw=b"42")


@pytest.fixture
def mock_alpha_client(ok_response, height_response):
    """Alpha client that accepts everything."""
    client = MagicMock()
    client.broadcast_transaction.return_value = ok_response
    client.broadcast_transaction_bytes.return_value = ok_response
    client.get_height.return_value = height_response
    client.get_height_int.return_value = 42
    client.get_mempool.return_value = Response(status=200, body=[], raw=b"[]")
    client.get_governance_proposals.return_value = Response(
        status=200, body=[{"id": "prop-1", "status": "active"}], raw=b"[]"
    )
    client.get_governance_state.return_value = Response(status=200, body={"state": "active"}, raw=b"{}")
    client.get_committee.return_value = Response(status=200, body={"members": ["v1", "v2", "v3", "v4"]}, raw=b"{}")
    client.get_transaction_status.return_value = Response(status=200, body="confirmed", raw=b'"confirmed"')
    return client


@pytest.fixture
def mock_alpha_client_rejecting(rejection_response, height_response):
    """Alpha client that rejects all broadcast_transaction calls."""
    client = MagicMock()
    client.broadcast_transaction.return_value = rejection_response
    client.broadcast_transaction_bytes.return_value = rejection_response
    client.get_height.return_value = height_response
    client.get_height_int.return_value = 42
    client.get_mempool.return_value = Response(status=200, body=[], raw=b"[]")
    client.get_governance_proposals.return_value = Response(status=200, body=[], raw=b"[]")
    client.get_governance_state.return_value = Response(status=200, body={}, raw=b"{}")
    client.get_committee.return_value = Response(status=200, body={}, raw=b"{}")
    return client
