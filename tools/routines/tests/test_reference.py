import copy
import io
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from tools.routines import reference as r


class ReferenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows = r.catalog_rows()
        cls.bundles = [(row, r.REPO / "routines" / row["directory"]) for row in cls.rows]
        cls.refs = [r.read_json(d / "reference.json") for _, d in cls.bundles]
        cls.vectors = [r.read_json(d / "vectors.json") for _, d in cls.bundles]

    def reject(self, function, *args):
        with self.assertRaises(r.InvalidReference):
            function(*args)

    def test_bundles_and_independent_matrices_validate(self):
        self.assertEqual(r.validate_catalog(), [])
        self.assertEqual(len(self.rows), 2)
        for (row, _), ref in zip(self.bundles, self.refs):
            with self.subTest(id=row["id"]):
                self.assertGreater(r.validate_vectors(r.matrix_vectors(row), ref), 0)

    def test_deployments_remain_empty(self):
        value = r.read_json(r.REPO / "routines/generated-registry.json")
        self.assertEqual(value["deployments"], [])
        self.assertEqual({row["status"] for row in self.rows}, {"reference"})

    def test_missing_extra_and_invalid_input_values_are_rejected(self):
        frame = self.vectors[1]["scenarios"][0]["inputs"]
        for key, bad in (("cooling_loop", -1e-12), ("cooling_loop", 100),
                         ("cooling_loop", float("nan")), ("cooling_loop", float("inf")),
                         ("cooling_loop", 10**1000), ("cooling_loop", True),
                         ("cooling", 1), ("cooling", "false"), ("group_mode", 0),
                         ("group_mode", 7), ("group_mode", 1.0), ("group_mode", True),
                         ("occupied_min_flow", -1), ("occupied_min_flow", 1.1),
                         ("cooling_max_flow", 0.1), ("supply_temperature", -0.01),
                         ("zone_temperature", None)):
            with self.subTest(port=key, value=repr(bad)[:40]):
                self.reject(r.validate_frame, frame | {key: bad}, self.refs[1])
        for key in frame:
            bad = dict(frame); del bad[key]
            self.reject(r.validate_frame, bad, self.refs[1])
        self.reject(r.validate_frame, frame | {"undocumented": 1}, self.refs[1])

    def test_zero_tiny_positive_and_equal_limits_are_admissible(self):
        for h, c in ((0., -0.), (1e-12, 0.), (0., 1e-12), (1., 1.)):
            r.validate_frame(dict(heating_loop=h, cooling_loop=c), self.refs[0])
        frame = self.vectors[1]["scenarios"][0]["inputs"]
        r.validate_frame(frame | {"occupied_min_flow": 0., "cooling_max_flow": 0.}, self.refs[1])
        r.validate_frame(frame | {"occupied_min_flow": 1., "cooling_max_flow": 1.}, self.refs[1])

    def test_no_vacuous_or_partial_assertions(self):
        changes = (
            lambda v: v.update(scenarios=[]),
            lambda v: v["scenarios"][0].update(expect=[]),
            lambda v: v["scenarios"][0]["expect"].pop(),
            lambda v: v["scenarios"][0]["expect"].append(copy.deepcopy(v["scenarios"][0]["expect"][0])),
            lambda v: v["scenarios"][0]["expect"][0].update(from_s=0.1, to_s=0.2),
            lambda v: v["scenarios"][0]["expect"][0].update(from_s=1, to_s=2),
            lambda v: v["scenarios"][0]["expect"][0].update(output="unknown"),
            lambda v: v["scenarios"][0]["expect"][0].update(equals=1),
            lambda v: v["scenarios"][0]["expect"][0].update(tolerance=1),
            lambda v: v["scenarios"].append(copy.deepcopy(v["scenarios"][0])),
        )
        for i, change in enumerate(changes):
            with self.subTest(mutation=i):
                value = copy.deepcopy(self.vectors[0]); change(value)
                self.reject(r.validate_vectors, value, self.refs[0])

    def test_clock_and_schedule_boundaries(self):
        for clock in ({"step_s": 0, "horizon_s": 2}, {"step_s": 1, "horizon_s": -1},
                      {"step_s": 1, "horizon_s": 1.5}, {"step_s": 1, "horizon_s": 10001},
                      {"step_s": float("nan"), "horizon_s": 2}):
            value = copy.deepcopy(self.vectors[0]); value["clock"] = clock
            self.reject(r.validate_vectors, value, self.refs[0])
        schedules = ([], [{"t": 1, "value": 0.}], [{"t": 0, "value": 0.}, {"t": 0, "value": 1.}],
                     [{"t": 0, "value": 0.}, {"t": 0.5, "value": 1.}],
                     [{"t": 0, "value": 0.}, {"t": 3, "value": 1.}],
                     [{"t": 0, "value": 0.}, {"t": 2, "value": 1.}, {"t": 1, "value": 0.}])
        for schedule in schedules:
            value = copy.deepcopy(self.vectors[0]); value["scenarios"][0]["inputs"]["heating_loop"] = schedule
            self.reject(r.validate_vectors, value, self.refs[0])

    def test_later_software_limit_violation_is_rejected(self):
        value = copy.deepcopy(self.vectors[1])
        value["scenarios"][0]["inputs"]["occupied_min_flow"] = [{"t": 0, "value": .2}, {"t": 1, "value": 2.}]
        self.reject(r.validate_vectors, value, self.refs[1])

    def test_strict_json_and_path_handling(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); p = root / "input.json"
            for text in ('{"x":0,"x":1}', '{"x":NaN}', '{"x":Infinity}', '{"x":1e309}', '[]'):
                p.write_text(text)
                self.reject(r.read_json, p)
            for relative in ("", "/etc/passwd", "../outside", "g36//class", "g36/./class", "g36\\class", "g36/\nclass"):
                self.reject(r.safe_path, root, relative)
            (root / "escape").symlink_to(root.parent)
            self.reject(r.safe_path, root, "escape/other")

    def test_cxf_rejects_duplicates_unconnected_ports_and_hidden_hysteresis(self):
        graph = r.read_json(self.bundles[0][1] / "reference.cxf.jsonld")
        value = copy.deepcopy(graph); value["@graph"].append(copy.deepcopy(value["@graph"][0]))
        self.reject(r.validate_graph, value, self.refs[0])
        value = copy.deepcopy(graph)
        next(n for n in value["@graph"] if n["@id"].endswith(".heat_positive.h"))["S231:value"]["@value"] = "0.01"
        self.reject(r.validate_graph, value, self.refs[0])
        value = copy.deepcopy(graph)
        next(n for n in value["@graph"] if n["@id"].endswith(".heating_loop")).pop("S231:isConnectedTo")
        self.reject(r.validate_graph, value, self.refs[0])
        value = copy.deepcopy(graph)
        value["@graph"][0].pop("S231:hasInstance")
        self.reject(r.validate_graph, value, self.refs[0])
        value = copy.deepcopy(graph)
        next(n for n in value["@graph"] if n["@id"].endswith(".heat_positive"))["@type"] = "CDL.Logical.TrueDelay"
        self.reject(r.validate_graph, value, self.refs[0])

    def test_reference_row_cannot_claim_deployment_or_escape(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); (root / "routines").mkdir()
            registry = {"schema": r.REGISTRY_SCHEMA, "routines": [self.rows[0]]}
            for change in ({"status": "verified"}, {"directory": "../outside"}, {"id": "G36-SCOPE-05-03"}):
                value = copy.deepcopy(registry); value["routines"][0].update(change)
                (root / "routines/registry.json").write_text(json.dumps(value))
                self.reject(r.catalog_rows, root)
            value = copy.deepcopy(registry); value["routines"].append(value["routines"][0])
            (root / "routines/registry.json").write_text(json.dumps(value))
            self.reject(r.catalog_rows, root)

    def test_engine_trace_must_contain_real_evidence(self):
        vector = copy.deepcopy(self.vectors[0]); vector["scenarios"] = vector["scenarios"][:1]
        out = {e["output"]: e["equals"] for e in vector["scenarios"][0]["expect"]}
        trace = {"schema": "cxf-library/replay-trace/v1", "scenarios": [{"name": "idle", "samples": [{"t": t, "outputs": out.copy()} for t in range(3)]}]}
        def run(value):
            with patch.object(r.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout=json.dumps(value), stderr="")):
                return r.replay(Path("unused"), self.bundles[0][1] / "reference.cxf.jsonld", vector, self.refs[0])
        self.assertEqual(run(trace)["assertions"], 12)
        for invalid in ([], None, {}, {"schema": "cxf-library/replay-trace/v1", "scenarios": None}):
            self.reject(run, invalid)
        for mutation in (lambda t: t.update(scenarios=[]),
                         lambda t: t["scenarios"][0].pop("name"),
                         lambda t: t["scenarios"][0]["samples"].__setitem__(0, None),
                         lambda t: t["scenarios"][0]["samples"].pop(),
                         lambda t: t["scenarios"][0]["samples"][0]["outputs"].update(heating=True),
                         lambda t: t["scenarios"][0]["samples"][0]["outputs"].pop("deadband")):
            value = copy.deepcopy(trace); mutation(value)
            self.reject(run, value)

    def test_empty_catalog_does_not_claim_engine_replay(self):
        with patch.object(r, "validate_catalog", return_value=[]), \
             patch.object(r, "catalog_rows", return_value=[]), \
             patch.object(r.sys, "argv", ["reference.py", "--verifier", "unused"]), \
             patch.object(r.sys, "stderr", new_callable=io.StringIO) as errors:
            self.assertEqual(r.main(), 1)
            self.assertIn("no engine replay was performed", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
