"""Tests for key_loader.py"""
import pytest
from key_loader import load, make_stub, KeySet, KeyEntry


class TestMakeStub:
    def test_has_governor_keys(self, stub_keys):
        assert len(stub_keys.governor_keys) == 5

    def test_has_validator_keys(self, stub_keys):
        assert len(stub_keys.validator_keys) == 5

    def test_has_funded_wallets(self, stub_keys):
        assert len(stub_keys.funded_wallets) == 20

    def test_has_prover_key(self, stub_keys):
        assert stub_keys.prover_key is not None

    def test_deploy_id(self, stub_keys):
        assert stub_keys.deploy_id == "test-deploy-001"

    def test_key_address_format(self, stub_keys):
        for k in stub_keys.governor_keys:
            assert k.alpha_addr.startswith("ac1")
        for k in stub_keys.validator_keys:
            assert k.alpha_addr.startswith("ac1")
        for k in stub_keys.funded_wallets:
            assert k.alpha_addr.startswith("ac1")

    def test_private_key_format(self, stub_keys):
        for k in stub_keys.funded_wallets:
            assert k.private_key.startswith("ap1")


class TestLoad:
    def test_load_yaml(self, sample_key_yaml):
        ks = load(str(sample_key_yaml))
        assert ks.deploy_id == "test001"
        assert len(ks.governor_keys) == 1
        assert len(ks.validator_keys) == 1
        assert len(ks.funded_wallets) == 2
        assert ks.prover_key is not None

    def test_governor_addr(self, sample_key_yaml):
        ks = load(str(sample_key_yaml))
        assert ks.governor_keys[0].alpha_addr == "ac1gov000000000000000000000000000000000000000000"

    def test_prover_key_addr(self, sample_key_yaml):
        ks = load(str(sample_key_yaml))
        assert ks.prover_key.alpha_addr == "ac1prv000000000000000000000000000000000000000000"

    def test_balances(self, sample_key_yaml):
        ks = load(str(sample_key_yaml))
        assert ks.funded_wallets[0].ax_balance == 1_000_000
        assert ks.funded_wallets[0].dx_balance == 500_000

    def test_missing_file_raises(self, tmp_path):
        with pytest.raises(FileNotFoundError):
            load(str(tmp_path / "nonexistent.yaml"))


class TestResolve:
    def test_funded_wallet_by_index(self, stub_keys):
        result = stub_keys.resolve("keys.funded_wallets[0]")
        assert isinstance(result, dict)
        assert "alpha_addr" in result

    def test_funded_wallet_field(self, stub_keys):
        addr = stub_keys.resolve("keys.funded_wallets[2].alpha_addr")
        assert isinstance(addr, str)
        assert addr.startswith("ac1")

    def test_governor_key(self, stub_keys):
        result = stub_keys.resolve("keys.governor_keys[0]")
        assert isinstance(result, dict)

    def test_prover_key(self, stub_keys):
        result = stub_keys.resolve("keys.prover_key")
        assert isinstance(result, dict)
        assert "alpha_addr" in result

    def test_prover_key_field(self, stub_keys):
        addr = stub_keys.resolve("keys.prover_key.alpha_addr")
        assert isinstance(addr, str)

    def test_non_keys_prefix_returns_none(self, stub_keys):
        result = stub_keys.resolve("other.field")
        assert result is None

    def test_out_of_range_index(self, stub_keys):
        result = stub_keys.resolve("keys.funded_wallets[999].alpha_addr")
        assert result is None


class TestWalletForRole:
    def test_governor_role(self, stub_keys):
        k = stub_keys.wallet_for_role("governor")
        assert k is not None
        assert k in stub_keys.governor_keys

    def test_validator_role(self, stub_keys):
        k = stub_keys.wallet_for_role("validator")
        assert k in stub_keys.validator_keys

    def test_general_user_role(self, stub_keys):
        k = stub_keys.wallet_for_role("general_user")
        assert k in stub_keys.funded_wallets

    def test_index_wraps(self, stub_keys):
        # index beyond list wraps via modulo
        k = stub_keys.wallet_for_role("governor", 100)
        assert k in stub_keys.governor_keys

    def test_prover_role(self, stub_keys):
        k = stub_keys.wallet_for_role("prover")
        assert k == stub_keys.prover_key


class TestKeyEntryToDict:
    def test_roundtrip(self, stub_keys):
        k = stub_keys.funded_wallets[0]
        d = k.to_dict()
        assert d["alpha_addr"] == k.alpha_addr
        assert d["private_key"] == k.private_key
        assert d["index"] == k.index
