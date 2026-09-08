#!/usr/bin/env python3
"""Emit two independently authored, algebraic G36-2018 reference graphs.

This is a small graph author, NOT the Modelica routine compiler. It does not
create deployable inventory rows. Review source.md and the cards with the graphs.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "authoring"))
from cxfgen import Doc, XSD_D, XSD_I  # noqa: E402

CLASSES = {
    "G36-05-03-ZONE-STATE": "routines/g36/zones/thermal/zone-state",
    "G36-05-05-COOLING-AIRFLOW-SETPOINT": "routines/g36/terminal-units/cooling-only/airflow-setpoint",
}


class Graph(Doc):
    """The existing engine-compatible CXF graph shape, with explicit ownership."""

    def add(self, label: str, cls: str, inputs: dict[str, str], output: str,
            params: dict | None = None) -> None:
        self.block(label, cls, params=params, ins=list(inputs), outs=["y"])
        for port, kind in inputs.items():
            self.port(f"{label}.{port}", kind + "_in")
        self.port(f"{label}.y", output + "_out")

    def wire(self, source: str, *targets: str) -> None:
        node = next(n for n in self.nodes if n["@id"] == self._iri(source)["@id"])
        prior = node.get("S231:isConnectedTo", [])
        if isinstance(prior, dict):
            prior = [prior]
        node["S231:isConnectedTo"] = prior + [self._iri(t) for t in targets]

    def finish(self) -> dict:
        value = super().finish()
        # OBC CXF 8.5: parent instance explicitly owns children. Keep the typed
        # hasInput/hasOutput/hasParameter links used by the engine as well.
        for node in value["@graph"]:
            children = []
            for key in ("S231:containsBlock", "S231:hasInput", "S231:hasOutput", "S231:hasParameter"):
                refs = node.get(key, [])
                children.extend(refs if isinstance(refs, list) else [refs])
            if children:
                node["S231:hasInstance"] = children
        return value


def zone_state() -> dict:
    """G36-2018 5.3.5: both active is deadband, with a separate diagnostic."""
    d = Graph("g36-05-03-zone-state", "zone_state")
    for label in ("heat_positive", "cool_positive"):
        d.add(label, "Reals.GreaterThreshold", {"u": "real"}, "bool", {"t": "0", "h": "0"})
    for label in ("heat_zero", "cool_zero", "neither_state"):
        d.add(label, "Logical.Not", {"u": "bool"}, "bool")
    for label in ("heat_only", "cool_only", "both_positive"):
        d.add(label, "Logical.And", {"u1": "bool", "u2": "bool"}, "bool")
    d.add("either_state", "Logical.Or", {"u1": "bool", "u2": "bool"}, "bool")
    d.boundary_input("heating_loop", "real_in", ["heat_positive.u"])
    d.boundary_input("cooling_loop", "real_in", ["cool_positive.u"])
    for name in ("heating", "cooling", "deadband", "loop_conflict"):
        d.boundary_output(name, "bool_out")
    d.wire("heat_positive.y", "heat_zero.u", "heat_only.u1", "both_positive.u1")
    d.wire("cool_positive.y", "cool_zero.u", "cool_only.u1", "both_positive.u2")
    d.wire("cool_zero.y", "heat_only.u2")
    d.wire("heat_zero.y", "cool_only.u2")
    d.wire("heat_only.y", "heating", "either_state.u1")
    d.wire("cool_only.y", "cooling", "either_state.u2")
    d.wire("either_state.y", "neither_state.u")
    d.wire("neither_state.y", "deadband")
    d.wire("both_positive.y", "loop_conflict")
    return d.finish()


def airflow_setpoint() -> dict:
    """G36-2018 Table 5.5.4 and 5.5.5; upstream ventilation and loops excluded."""
    d = Graph("g36-05-05-cooling-airflow-setpoint", "airflow_setpoint")
    for label, value in (("occupied_code", 1), ("cooldown_code", 2), ("setup_code", 3)):
        d.add(label, "Integers.Sources.Constant", {}, "int", {"k": (str(value), XSD_I)})
    for label in ("occupied", "cooldown", "setup"):
        d.add(label, "Integers.Equal", {"u1": "int", "u2": "int"}, "bool")
        d.wire(label + "_code.y", label + ".u2")
    d.add("preconditioning", "Logical.Or", {"u1": "bool", "u2": "bool"}, "bool")
    d.add("cooling_mode", "Logical.Or", {"u1": "bool", "u2": "bool"}, "bool")
    d.add("zero", "Reals.Sources.Constant", {}, "real", {"k": "0"})
    for label in ("minimum", "maximum", "select_flow"):
        d.add(label, "Reals.Switch", {"u1": "real", "u2": "bool", "u3": "real"}, "real")
    d.add("span", "Reals.Subtract", {"u1": "real", "u2": "real"}, "real")
    d.add("modulation", "Reals.Multiply", {"u1": "real", "u2": "real"}, "real")
    d.add("mapped_flow", "Reals.Add", {"u1": "real", "u2": "real"}, "real")
    d.add("warm_air", "Reals.Greater", {"u1": "real", "u2": "real"}, "bool", {"h": "0"})
    d.add("not_warm_air", "Logical.Not", {"u": "bool"}, "bool")
    d.add("can_cool", "Logical.And", {"u1": "bool", "u2": "bool"}, "bool")
    d.boundary_input("group_mode", "int_in", ["occupied.u1", "cooldown.u1", "setup.u1"])
    d.boundary_input("cooling", "bool_in", ["can_cool.u1"])
    d.boundary_input("cooling_loop", "real_in", ["modulation.u1"])
    d.boundary_input("occupied_min_flow", "real_in", ["minimum.u1"])
    d.boundary_input("cooling_max_flow", "real_in", ["maximum.u1"])
    d.boundary_input("supply_temperature", "real_in", ["warm_air.u1"])
    d.boundary_input("zone_temperature", "real_in", ["warm_air.u2"])
    for name in ("active_min_flow", "active_max_flow", "airflow_setpoint"):
        d.boundary_output(name, "real_out")
    d.boundary_output("warm_supply_air", "bool_out")
    d.wire("occupied.y", "minimum.u2", "cooling_mode.u1")
    d.wire("cooldown.y", "preconditioning.u1")
    d.wire("setup.y", "preconditioning.u2")
    d.wire("preconditioning.y", "cooling_mode.u2")
    d.wire("cooling_mode.y", "maximum.u2")
    d.wire("zero.y", "minimum.u3", "maximum.u3")
    d.wire("minimum.y", "active_min_flow", "span.u2", "mapped_flow.u1", "select_flow.u3")
    d.wire("maximum.y", "active_max_flow", "span.u1")
    d.wire("span.y", "modulation.u2")
    d.wire("modulation.y", "mapped_flow.u2")
    d.wire("mapped_flow.y", "select_flow.u1")
    d.wire("warm_air.y", "warm_supply_air", "not_warm_air.u")
    d.wire("not_warm_air.y", "can_cool.u2")
    d.wire("can_cool.y", "select_flow.u2")
    d.wire("select_flow.y", "airflow_setpoint")
    return d.finish()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compare generated graphs without writing")
    args = parser.parse_args()
    failures = []
    for (cid, directory), factory in zip(CLASSES.items(), (zone_state, airflow_setpoint)):
        path = REPO / directory / "reference.cxf.jsonld"
        text = json.dumps(factory(), indent=2, allow_nan=False) + "\n"
        if args.check:
            if not path.is_file() or path.read_text(encoding="utf-8") != text:
                failures.append(str(path.relative_to(REPO)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
    if failures:
        print("regenerate reference graphs: " + ", ".join(failures), file=sys.stderr)
        return 1
    print("2 reference CXF graphs " + ("match the graph author" if args.check else "written"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
