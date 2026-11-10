import csv
import tempfile
import unittest
from pathlib import Path

from tools.simharness import harness


class TimelineTests(unittest.TestCase):
    def setUp(self):
        self.old_step = harness.STEP_S
        harness.STEP_S = 3600

    def tearDown(self):
        harness.STEP_S = self.old_step

    @staticmethod
    def write_day(path: Path, month: int, day: int) -> None:
        with path.open("w", newline="") as f:
            rows = csv.writer(f)
            rows.writerow(["Date/Time", "value"])
            for hour in range(1, 25):
                rows.writerow([f" {month:02}/{day:02}  {hour:02}:00:00", hour])

    def test_exact_requested_period_is_accepted(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "eplusout.csv"
            self.write_day(path, 7, 6)
            result = harness.validate_csv_timeline(path, (7, 6), (7, 6))
            self.assertEqual(result["samples"], 24)
            self.assertEqual(result["observed_last"], "07/06  24:00:00")

    def test_reused_csv_cannot_be_relabelled(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "eplusout.csv"
            self.write_day(path, 7, 6)
            with self.assertRaisesRegex(ValueError, "observed period does not match"):
                harness.validate_csv_timeline(path, (1, 6), (1, 6))

    def test_missing_runperiod_is_created(self):
        model = {}
        harness.configure_period_and_timestep(model, (1, 6), (1, 12))
        self.assertEqual(len(model["RunPeriod"]), 1)
        period = next(iter(model["RunPeriod"].values()))
        self.assertEqual(period["begin_month"], 1)
        self.assertEqual(period["end_day_of_month"], 12)


class ReplayResultTests(unittest.TestCase):
    def test_relative_and_absolute_replay_paths_are_isolated(self):
        log = """relative/replay/TOWER-0005__one
  PASS scenario_one
/tmp/out/replay/CHW-0009__one
  FAIL scenario_two
relative/replay/TOWER-0005__two
  FAIL scenario_three
"""
        self.assertEqual(harness.replay_results(log), {
            "TOWER-0005__one": "pass",
            "CHW-0009__one": "fail",
            "TOWER-0005__two": "fail",
        })


class PlantTopologyTests(unittest.TestCase):
    def test_hw_fleet_comes_only_from_target_supply_branches(self):
        model = {
            "PlantLoop": {
                "HeatSys1": {
                    "fluid_type": "Water",
                    "plant_side_outlet_node_name": "out",
                    "plant_side_inlet_node_name": "in",
                    "loop_temperature_setpoint_node_name": "sp",
                    "plant_side_branch_list_name": "heat branches",
                }
            },
            "BranchList": {
                "heat branches": {
                    "branches": [{"branch_name": "heat equipment"}]
                }
            },
            "Branch": {
                "heat equipment": {
                    "components": [
                        {"component_object_type": "Pump:VariableSpeed",
                         "component_name": "Heat Pump"},
                        {"component_object_type": "Boiler:HotWater",
                         "component_name": "Heat Boiler"},
                    ]
                },
                "other equipment": {
                    "components": [
                        {"component_object_type": "Boiler:HotWater",
                         "component_name": "Dummy Boiler"}
                    ]
                },
            },
            "Boiler:HotWater": {"Heat Boiler": {}, "Dummy Boiler": {}},
            "Pump:VariableSpeed": {"Heat Pump": {}},
        }
        nodes = harness.plant_nodes(model)
        self.assertEqual(nodes["hw"]["boilers"], ["Heat Boiler"])
        self.assertEqual(nodes["hw"]["pumps"], ["Heat Pump"])

    def test_tracking_gate_requires_boiler_pump_and_stable_setpoint(self):
        old_step = harness.STEP_S
        old_lead = harness.OS_LEAD_S
        harness.STEP_S = 60
        harness.OS_LEAD_S = 120
        try:
            pts = {
                "boiler_status": [True] * 70,
                "hw_pump_status": [True] * 70,
                "hws_temp_sp": [60.0] * 35 + [55.0] * 35,
            }
            self.assertEqual(
                harness.plant_gate_windows("hw_tracking", pts, 70),
                [(120, 2040), (2220, 4140)],
            )
            pts["hw_pump_status"] = [False] * 70
            self.assertEqual(
                harness.plant_gate_windows("hw_tracking", pts, 70), [])
        finally:
            harness.STEP_S = old_step
            harness.OS_LEAD_S = old_lead


class UngatedWindowTests(unittest.TestCase):
    def setUp(self):
        self.old_step = harness.STEP_S
        harness.STEP_S = 300

    def tearDown(self):
        harness.STEP_S = self.old_step

    def test_ungated_rule_is_asserted_from_the_lead_margin_to_the_last_sample(self):
        self.assertEqual(harness.GATE_LEAD_S, 3600)
        # A week at 300 s: 2,017 samples, the last at 604,800 s.
        self.assertEqual(
            harness.plant_gate_windows(None, {}, 2017), [(3600, 604800)])
        # The shortest run that covers the window: its last sample is the one tick inside it.
        self.assertEqual(
            harness.plant_gate_windows(None, {}, 13), [(3600, 3600)])

    def test_run_that_ends_inside_the_lead_margin_is_refused_and_says_why(self):
        # One sample short of the margin, and a single sample: both used to
        # get the inverted window (3600, last sample).
        for n, last in ((12, 3300), (2, 300), (1, 0)):
            with self.subTest(samples=n):
                with self.assertRaisesRegex(
                        ValueError,
                        rf"run too short for an ungated rule: {n} sample\(s\) at 300s "
                        rf"end at {last}s, before the 3600s lead margin is over, so the "
                        rf"window 3600\.\.{last}s would hold no tick and compare nothing"):
                    harness.plant_gate_windows(None, {}, n)

    def test_gated_rule_on_a_short_run_still_has_no_window_and_no_refusal(self):
        pts = {"chiller_load": [50.0] * 12}
        self.assertEqual(harness.plant_gate_windows("chw_load40", pts, 12), [])


if __name__ == "__main__":
    unittest.main()
