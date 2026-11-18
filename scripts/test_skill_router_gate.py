#!/usr/bin/env python3
"""Tests for `scripts/skill_router_gate.py`, run by the CI job `script-tests`."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("skill_router_gate", HERE / "skill_router_gate.py")
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)


class Routing(unittest.TestCase):
    def test_cri_task_routes_to_cri_cni(self):
        self.assertEqual(gate.route("validate CRI conformance and kubelet logs"), "cri-cni")

    def test_docker_task_routes_to_docker_podman(self):
        self.assertEqual(gate.route("check Docker Engine API and Compose parity"), "docker-podman")

    def test_vm_task_routes_to_vm_storage(self):
        self.assertEqual(gate.route("microVM snapshot and restore"), "vm-storage")

    def test_cve_task_routes_to_cve_mcp(self):
        self.assertEqual(gate.route("scan images for CVEs and check SBOM"), "cve-mcp")

    def test_unknown_defaults_to_linux_oci(self):
        self.assertEqual(gate.route("something unspecified"), "linux-oci")

    def test_every_route_target_is_a_required_domain(self):
        for _, keys in gate._ROUTE:
            self.assertIn(gate.route(keys[0]), gate.REQUIRED_DOMAINS)


class ExternalDependency(unittest.TestCase):
    def test_paas_is_external(self):
        self.assertEqual(gate.external_dependency("paas"), "ngolacloud-control-plane")

    def test_core_domains_have_no_external_dependency(self):
        for d in ("linux-oci", "cri-cni", "vm-storage"):
            self.assertIsNone(gate.external_dependency(d))


class RepoState(unittest.TestCase):
    def test_gate_passes_on_the_repo(self):
        self.assertEqual(gate.main([]), 0)

    def test_no_errors(self):
        self.assertEqual(gate.check(), [])

    def test_router_covers_every_required_domain(self):
        text = gate.E2E_ROUTER.read_text(encoding="utf-8")
        for dom in gate.REQUIRED_DOMAINS:
            self.assertIn(dom, text, dom)

    def test_harness_check_targets_exist(self):
        # make ci / make gates must be real Makefile targets.
        targets = gate._makefile_targets()
        self.assertIn("ci", targets)
        self.assertIn("gates", targets)


if __name__ == "__main__":
    unittest.main()
