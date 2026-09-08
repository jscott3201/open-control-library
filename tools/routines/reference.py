#!/usr/bin/env python3
"""Validate and replay the explicitly non-deployable G36 reference catalog.

Replay uses the real Open Control Engine verifier, not a Python block evaluator.
The checks here are offline input admission and evidence hygiene, not a live host.
"""
from __future__ import annotations

import argparse
import itertools
import json
import math
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

REGISTRY_SCHEMA = "cxf-library/routine-registry/v3"
ARTIFACTS = ("class-manifest.json", "interface.json", "specialization.schema.json",
             "specialization.json", "reference.json", "reference.cxf.jsonld",
             "vectors.json", "card.md", "source.md", "diagram.svg", "overview.svg")
ID_PATTERN = re.compile(r"G36-05-(0[1-9]|1[0-9]|2[0-2])-[A-Z]+(?:-[A-Z]+)*\Z")


class InvalidReference(ValueError):
    """An artifact or replay input is outside the declared reference profile."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise InvalidReference(message)


def exact(value, keys, label: str) -> None:
    expected = set(keys)
    require(isinstance(value, dict), f"{label}: must be an object")
    missing = sorted(expected - set(value))
    extra = sorted(set(value) - expected)
    details = "; ".join((["missing " + ", ".join(missing)] if missing else []) +
                        (["unexpected " + ", ".join(extra)] if extra else []))
    require(not missing and not extra, f"{label}: keys must be exactly {', '.join(keys)} ({details})")


def number(value) -> bool:
    try:
        return type(value) in (int, float) and math.isfinite(value)
    except OverflowError:
        return False


def read_json(path: Path) -> dict:
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    def finite_float(text):
        result = float(text)
        require(math.isfinite(result), "non-finite JSON number")
        return result

    def bad_constant(text):
        raise InvalidReference(f"non-finite JSON number {text}")

    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=pairs,
                       parse_float=finite_float, parse_constant=bad_constant)
    require(isinstance(value, dict), f"{path.name}: must contain a JSON object")
    return value


def safe_path(root: Path, relative: str) -> Path:
    require(isinstance(relative, str) and relative != "", "path must be nonempty text")
    require(not relative.startswith("/") and "\\" not in relative and
            all(p not in ("", ".", "..") for p in relative.split("/")) and
            all(ord(c) >= 32 and ord(c) != 127 for c in relative), "unsafe relative path")
    path = root / relative
    require(path.resolve().is_relative_to(root.resolve()), "path escapes repository")
    return path


def catalog_rows(root: Path = REPO) -> list[dict]:
    value = read_json(root / "routines/registry.json")
    exact(value, ("schema", "routines"), "routines/registry.json")
    require(value["schema"] == REGISTRY_SCHEMA, f"routines/registry.json: schema must be {REGISTRY_SCHEMA!r}")
    require(isinstance(value["routines"], list), "routines/registry.json: routines must be an array")
    ids, paths = set(), set()
    for row in value["routines"]:
        exact(row, ("id", "name", "status", "directory"), "routine row")
        require(isinstance(row["id"], str) and ID_PATTERN.fullmatch(row["id"]) is not None, "invalid canonical routine id")
        require(row["id"] not in ids, "duplicate canonical routine id")
        ids.add(row["id"])
        require(isinstance(row["name"], str) and row["name"].strip() == row["name"] and row["name"], "invalid routine name")
        require(row["status"] == "reference", "routine status must be reference; deployment is not qualified")
        directory = safe_path(root / "routines", row["directory"])
        require(row["directory"].startswith("g36/"), "routine directory must be below g36/")
        require(directory.resolve() not in paths, "duplicate routine directory")
        paths.add(directory.resolve())
    return value["routines"]


def spec_valid(spec: dict, label: str) -> None:
    require(isinstance(spec, dict), f"{label}: port specification must be an object")
    kind = spec.get("type")
    if kind == "boolean":
        exact(spec, ("type",), label)
    elif kind == "integer":
        exact(spec, ("type", "enum"), label)
        enum = spec["enum"]
        require(isinstance(enum, dict) and enum and all(isinstance(k, str) and k for k in enum), f"{label}: enum symbols required")
        codes = list(enum.values())
        require(all(type(c) is int for c in codes) and codes == list(range(1, len(codes) + 1)), f"{label}: enum ABI must use ordered unique integers starting at one")
    elif kind == "real":
        require(set(spec) <= {"type", "unit", "minimum", "maximum"} and "unit" in spec, f"{label}: invalid Real specification")
        require(spec["unit"] in ("1", "m3/s", "K"), f"{label}: unsupported reference unit")
        for key in ("minimum", "maximum"):
            require(key not in spec or number(spec[key]), f"{label}: finite {key} required")
        if "minimum" in spec and "maximum" in spec:
            require(spec["minimum"] <= spec["maximum"], f"{label}: reversed bounds")
    else:
        raise InvalidReference(f"{label}: unsupported scalar type")


def value_valid(value, spec: dict, label: str) -> None:
    if spec["type"] == "boolean":
        require(type(value) is bool, f"{label}: Boolean required, not numeric coercion")
    elif spec["type"] == "integer":
        require(type(value) is int and value in spec["enum"].values(), f"{label}: unknown or non-integer enum value")
    else:
        require(number(value), f"{label}: finite Real required")
        require("minimum" not in spec or value >= spec["minimum"], f"{label}: below minimum")
        require("maximum" not in spec or value <= spec["maximum"], f"{label}: above maximum")


def validate_frame(frame: dict, ref: dict) -> None:
    exact(frame, ref["inputs"], "input frame")
    for name, spec in ref["inputs"].items():
        value_valid(frame[name], spec, name)
    for lower, upper in ref["ordered_inputs"]:
        require(frame[lower] <= frame[upper], f"{lower} exceeds {upper}")


def validate_vectors(vectors: dict, ref: dict) -> int:
    exact(vectors, ("schema", "clock", "scenarios"), "vectors")
    require(vectors["schema"] == "cxf-library/vectors/v1", "invalid vectors schema")
    exact(vectors["clock"], ("step_s", "horizon_s"), "clock")
    step, horizon = vectors["clock"]["step_s"], vectors["clock"]["horizon_s"]
    require(number(step) and step > 0 and number(horizon) and horizon >= 0, "invalid replay clock")
    ratio = horizon / step
    require(math.isfinite(ratio) and ratio <= 10000 and ratio.is_integer(), "clock horizon must be an integral, bounded number of steps")
    ticks = [i * step for i in range(int(ratio) + 1)]
    cases = vectors["scenarios"]
    require(isinstance(cases, list) and 0 < len(cases) <= 512, "1..512 nonempty scenarios required")
    require(len(cases) * len(ticks) <= 1000000, "replay exceeds sample budget")
    names, assertions = set(), 0
    for case in cases:
        require(isinstance(case, dict) and {"name", "inputs", "expect"} <= set(case) <= {"name", "description", "inputs", "expect"}, "invalid scenario shape")
        name = case["name"]
        require(isinstance(name, str) and name.strip() and name not in names, "empty or duplicate scenario name")
        names.add(name)
        exact(case["inputs"], ref["inputs"], f"{name} inputs")
        schedules = {}
        for port, value in case["inputs"].items():
            events = value if isinstance(value, list) else [{"t": 0, "value": value}]
            require(events, f"{name}/{port}: empty input schedule")
            previous = -1
            for event in events:
                exact(event, ("t", "value"), f"{name}/{port} event")
                t = event["t"]
                require(number(t) and t in ticks and t > previous, f"{name}/{port}: event times must be ordered unique clock ticks")
                value_valid(event["value"], ref["inputs"][port], f"{name}/{port}")
                previous = t
            require(events[0]["t"] == 0, f"{name}/{port}: initial value at t=0 required")
            schedules[port] = events
        expected = case["expect"]
        require(isinstance(expected, list) and expected, f"{name}: expectations required")
        for exp in expected:
            require(isinstance(exp, dict) and {"output", "from_s", "to_s", "equals"} <= set(exp) <= {"output", "from_s", "to_s", "equals", "tolerance"}, f"{name}: invalid expectation shape")
            output = exp["output"]
            require(isinstance(output, str) and output in ref["outputs"], f"{name}: unknown output")
            value_valid(exp["equals"], ref["outputs"][output], f"{name}/{output} expected")
            lo, hi = exp["from_s"], exp["to_s"]
            require(number(lo) and number(hi) and lo in ticks and hi in ticks and lo <= hi, f"{name}: empty or off-clock expectation window")
            tol = exp.get("tolerance", 1e-9)
            require(number(tol) and 0 <= tol <= 1e-6, f"{name}: tolerance must be finite and between 0 and 1e-6")
            require("tolerance" not in exp or ref["outputs"][output]["type"] == "real", f"{name}: tolerance only applies to Real outputs")
        for t in ticks:
            frame = {p: next(e["value"] for e in reversed(events) if e["t"] <= t) for p, events in schedules.items()}
            validate_frame(frame, ref)
            for output in ref["outputs"]:
                matches = [e for e in expected if e["output"] == output and e["from_s"] <= t <= e["to_s"]]
                require(len(matches) == 1, f"{name}/{output}@{t}: exactly one assertion required; no gaps or overlaps")
                assertions += 1
    return assertions


def validate_graph(graph: dict, ref: dict) -> None:
    nodes = graph.get("@graph")
    require(isinstance(nodes, list) and nodes, "CXF @graph must be nonempty")
    require(all(isinstance(n, dict) and isinstance(n.get("@id"), str) for n in nodes), "invalid CXF node")
    by_id = {n["@id"]: n for n in nodes}
    require(len(by_id) == len(nodes), "duplicate CXF node id")
    root = by_id.get(ref["root_id"])
    require(root is not None, "reference root id absent from CXF")
    def refs(value):
        values = value if isinstance(value, list) else [value]
        require(all(isinstance(v, dict) and set(v) == {"@id"} and isinstance(v["@id"], str) for v in values), "invalid CXF reference")
        return [v["@id"] for v in values]
    kinds = {"real": "Real", "integer": "Integer", "boolean": "Boolean"}
    boundary_in, boundary_out = set(), set()
    for direction, predicate in (("inputs", "S231:hasInput"), ("outputs", "S231:hasOutput")):
        expected = {ref["root_id"] + "." + p for p in ref[direction]}
        require(set(refs(root.get(predicate, []))) == expected, "CXF boundary differs from reference contract")
        (boundary_in if direction == "inputs" else boundary_out).update(expected)
        for port, spec in ref[direction].items():
            node = by_id.get(ref["root_id"] + "." + port, {})
            require(node.get("@type") == "S231:" + kinds[spec["type"]] + ("Input" if direction == "inputs" else "Output"), "CXF boundary type mismatch")
    declared, inputs, outputs = {ref["root_id"]}, set(), set()
    allowed = {"Logical.Not", "Logical.And", "Logical.Or", "Reals.Greater", "Reals.GreaterThreshold",
               "Reals.Sources.Constant", "Integers.Sources.Constant", "Integers.Equal", "Reals.Switch",
               "Reals.Subtract", "Reals.Multiply", "Reals.Add"}
    for bid in refs(root.get("S231:containsBlock", [])):
        block = by_id.get(bid)
        require(block is not None and block.get("@type", "").split(".CDL.")[-1] in allowed, "unsupported reference block class")
        declared.add(bid)
        for pred in ("S231:hasInput", "S231:hasOutput", "S231:hasParameter"):
            children = refs(block.get(pred, []))
            require(all(c in by_id for c in children), "dangling child instance")
            declared.update(children)
            if pred == "S231:hasInput": inputs.update(children)
            if pred == "S231:hasOutput": outputs.update(children)
        if block["@type"].endswith(("Reals.Greater", "Reals.GreaterThreshold")):
            require(float(by_id.get(bid + ".h", {}).get("S231:value", {}).get("@value", "nan")) == 0, "reference comparators must explicitly have zero hysteresis")
    declared.update(boundary_in | boundary_out)
    require(declared == set(by_id), "unowned or missing CXF node")
    drivers = {}
    for node in nodes:
        for pred in ("S231:hasInput", "S231:hasOutput", "S231:hasParameter", "S231:containsBlock"):
            if pred in node:
                require(set(refs(node[pred])) <= set(refs(node.get("S231:hasInstance", []))), "missing CXF hasInstance ownership")
        if "S231:isConnectedTo" not in node: continue
        source = node["@id"]
        require(source in outputs | boundary_in, "invalid CXF connection source")
        for target in refs(node["S231:isConnectedTo"]):
            require(target in inputs | boundary_out, "invalid CXF connection destination")
            require(target not in drivers, "multiply driven CXF connector")
            require(node.get("S231:isOfDataType") == by_id[target].get("S231:isOfDataType"), "cross-type CXF connection")
            drivers[target] = source
    require(set(drivers) == inputs | boundary_out, "unconnected CXF input or output boundary")


def validate_bundle(root: Path, row: dict) -> dict:
    from tools.lint import routine_schemas as schemas
    directory = safe_path(root / "routines", row["directory"])
    for name in ARTIFACTS:
        path = safe_path(directory, name)
        require(path.is_file(), f"{row['id']}: missing {name}")
    errors = []
    by_id, registry = schemas._load_schemas(root, errors)
    require(not errors, "; ".join(errors))
    values = {}
    for filename, schema_id in schemas.FIXTURE_SCHEMAS.items():
        value = read_json(directory / filename)
        schemas._check_schema_instance(value, schema_id, str(directory / filename), by_id, registry, errors)
        values[filename] = value
    require(not errors, "; ".join(errors))
    manifest, interface, specialization = (values[k] for k in ("class-manifest.json", "interface.json", "specialization.json"))
    schemas._check_manifest(manifest, errors)
    schemas._check_interface_and_specialization(interface, specialization, errors, str(directory / "interface.json"), str(directory / "specialization.json"))
    schemas._check_cross_document(manifest, interface, specialization, errors)
    require(not errors, "; ".join(errors))
    require(manifest["id"] == row["id"], "catalog/manifest id mismatch")
    require(manifest["source"]["kind"] == "independent", "reference profile currently supports independent authoring only")
    for source in manifest["source"]["paths"]:
        require(safe_path(root, source).is_file(), "missing independent source")
    for artifact in manifest["artifacts"].values():
        require(safe_path(root, artifact).parent == directory, "manifest artifacts must use registered class directory")
    spec_schema = read_json(directory / "specialization.schema.json")
    require(spec_schema == {"$schema": schemas.DIALECT, "$ref": schemas.SPECIALIZATION_ID}, "reference specialization schema must reuse existing specialization contract")
    ref = read_json(directory / "reference.json")
    exact(ref, ("schema", "canonical_id", "revision", "standard", "clauses", "root_id", "inputs", "outputs", "ordered_inputs"), "reference")
    require(ref["schema"] == "cxf-library/routine-reference/v1" and ref["canonical_id"] == row["id"] and ref["revision"] == manifest["revision"], "reference identity mismatch")
    require(ref["standard"] == "ASHRAE Guideline 36-2018", "reference profile must identify the 2018 source baseline")
    require(isinstance(ref["clauses"], list) and ref["clauses"] and all(isinstance(c, str) and c.strip() for c in ref["clauses"]), "source clauses required")
    require(isinstance(ref["root_id"], str) and ref["root_id"].startswith("urn:cxf-library:" + row["id"].lower() + "#"), "invalid reference root")
    require(not interface["parameters"] and not interface["dimensions"], "reference profile currently supports scalar, runtime-configured classes only")
    types = {t["id"]: t for t in interface["types"]}
    for direction in ("inputs", "outputs"):
        ports = ref[direction]
        require(isinstance(ports, dict) and ports, "nonempty reference ports required")
        connectors = {c["id"]: c for c in interface["connectors"] if c["direction"] == direction[:-1]}
        require(set(ports) == set(connectors), "interface/reference connector mismatch")
        for name, spec in ports.items():
            spec_valid(spec, name)
            conn = connectors[name]
            require(conn["shape"] == {"kind": "scalar"} and conn["presence"] == {"kind": "always"}, "reference ports must be always-present scalars")
            use = conn["type"]
            typ = types[use["type"]] if use["kind"] == "named" else use
            require(typ.get("primitive", "integer" if typ.get("kind") == "enum" else "") == spec["type"], "interface/reference primitive mismatch")
            if "unit" in spec: require(typ.get("unit") == spec["unit"], "interface/reference unit mismatch")
            if "enum" in spec: require(typ.get("kind") == "enum" and [m["id"] for m in typ["members"]] == list(spec["enum"]), "interface/reference enum mismatch")
    require(isinstance(ref["ordered_inputs"], list), "ordered_inputs must be a list")
    for pair in ref["ordered_inputs"]:
        require(isinstance(pair, list) and len(pair) == 2 and all(isinstance(p, str) and p in ref["inputs"] for p in pair), "invalid ordered-input relation")
        require(all(ref["inputs"][p]["type"] == "real" for p in pair) and ref["inputs"][pair[0]]["unit"] == ref["inputs"][pair[1]]["unit"], "ordered inputs must be same-unit Reals")
    text = (directory / "card.md").read_text(encoding="utf-8")
    require(text.startswith("---\n") and "\n---\n" in text[4:], "missing card frontmatter")
    import yaml
    card = yaml.safe_load(text.split("---\n", 2)[1])
    require(isinstance(card, dict) and all(card.get(k) == row[k] for k in ("id", "name", "status")), "catalog/card mismatch")
    require(card.get("standard") == ref["standard"] and card.get("clauses") == ref["clauses"], "card/source baseline mismatch")
    validate_graph(read_json(directory / "reference.cxf.jsonld"), ref)
    validate_vectors(read_json(directory / "vectors.json"), ref)
    return ref


def validate_catalog(root: Path = REPO) -> list[str]:
    errors = []
    try:
        rows = catalog_rows(root)
    except (InvalidReference, OSError, ValueError, TypeError, OverflowError) as exc:
        return [str(exc)]
    for row in rows:
        try:
            validate_bundle(root, row)
        except (InvalidReference, OSError, ValueError, TypeError, KeyError, OverflowError) as exc:
            errors.append(f"{row['id']}: {exc}")
    registered = {(root / "routines" / row["directory"]).resolve() for row in rows}
    for filename in ARTIFACTS:
        for path in (root / "routines/g36").rglob(filename):
            # Only class-specific filenames participate; source inventory is separate.
            if path.parent.resolve() not in registered:
                errors.append(f"unregistered reference artifact: {path.relative_to(root)}")
    return sorted(errors)


def matrix_vectors(row: dict) -> dict:
    """Independent scalar oracle cases, not an implementation of CDL blocks."""
    scenarios = []
    def add(name, frame, expected):
        scenarios.append({"name": name, "inputs": frame, "expect": [
            {"output": key, "from_s": 0, "to_s": 0, "equals": value} for key, value in expected.items()]})
    if row["id"] == "G36-05-03-ZONE-STATE":
        for i, (h, c) in enumerate(itertools.product((0., 1e-12, .4, 1.), repeat=2)):
            # Truth-table interpretation of 5.3.5 on the admitted domain.
            state = "heating" if h != 0 and c == 0 else "cooling" if c != 0 and h == 0 else "deadband"
            add(f"state-matrix-{i}", {"heating_loop": h, "cooling_loop": c},
                {"heating": state == "heating", "cooling": state == "cooling", "deadband": state == "deadband", "loop_conflict": h != 0 and c != 0})
    elif row["id"] == "G36-05-05-COOLING-AIRFLOW-SETPOINT":
        cases = itertools.product(range(1, 7), (False, True), (0., .5, 1.), ((0., 0.), (.2, 1.), (1., 1.)), (-.01, 0., .01))
        for i, (mode, cooling, loop, limits, delta) in enumerate(cases):
            low, high = limits
            minimum = low if mode == 1 else 0.
            maximum = high if mode in (1, 2, 3) else 0.
            value = (1 - loop) * minimum + loop * maximum if cooling and delta <= 0 else minimum
            add(f"airflow-matrix-{i}", dict(group_mode=mode, cooling=cooling, cooling_loop=loop,
                occupied_min_flow=low, cooling_max_flow=high, supply_temperature=295. + delta, zone_temperature=295.),
                dict(active_min_flow=minimum, active_max_flow=maximum, airflow_setpoint=value, warm_supply_air=delta > 0))
    else:
        raise InvalidReference("new reference class needs an explicit matrix oracle")
    return {"schema": "cxf-library/vectors/v1", "clock": {"step_s": 1., "horizon_s": 0.}, "scenarios": scenarios}


def replay(verifier: Path, graph: Path, vectors: dict, ref: dict) -> dict:
    assertions = validate_vectors(vectors, ref)
    with tempfile.TemporaryDirectory(prefix="ocl-reference-") as temporary:
        directory = Path(temporary)
        shutil.copyfile(graph, directory / "rule.cxf.jsonld")
        vp = directory / "vectors.json"
        vp.write_text(json.dumps(vectors, allow_nan=False), encoding="utf-8")
        result = subprocess.run([str(verifier.resolve()), "--trace-json", str(directory), str(vp)], capture_output=True, text=True, timeout=120, check=False)
        require(result.returncode == 0, f"engine replay failed: {result.stderr or result.stdout}")
        trace = json.loads(result.stdout)
    require(isinstance(trace, dict) and trace.get("schema") == "cxf-library/replay-trace/v1", "unexpected engine trace schema")
    traces = trace.get("scenarios", [])
    require(isinstance(traces, list) and len(traces) == len(vectors["scenarios"]), "missing or extra engine scenarios")
    count = int(vectors["clock"]["horizon_s"] / vectors["clock"]["step_s"]) + 1
    for case, observed in zip(vectors["scenarios"], traces):
        require(isinstance(observed, dict) and case["name"] == observed.get("name") and isinstance(observed.get("samples"), list) and len(observed["samples"]) == count, "engine scenario or sample-count mismatch")
        for i, sample in enumerate(observed["samples"]):
            t = i * vectors["clock"]["step_s"]
            require(isinstance(sample, dict) and number(sample.get("t")) and sample["t"] == t and isinstance(sample.get("outputs"), dict) and set(sample["outputs"]) == set(ref["outputs"]), "engine clock/output boundary mismatch")
            for exp in case["expect"]:
                if not exp["from_s"] <= t <= exp["to_s"]: continue
                key = exp["output"]
                actual = sample["outputs"][key]
                value_valid(actual, ref["outputs"][key], f"engine/{key}")
                expected = exp["equals"]
                equal = abs(actual - expected) <= exp.get("tolerance", 1e-9) if ref["outputs"][key]["type"] == "real" else type(actual) is type(expected) and actual == expected
                require(equal, f"{case['name']}/{key}@{t}: expected {expected!r}, observed {actual!r}")
    return {"scenarios": len(traces), "samples": len(traces) * count, "assertions": assertions}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verifier", type=Path, help="existing cxf-verify binary; enables actual engine replay")
    parser.add_argument("--report", type=Path, help="write software evidence summary, never deployment eligibility")
    args = parser.parse_args()
    errors = validate_catalog(REPO)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    report = {"status": "reference-only", "production_deployment_qualified": False, "routines": []}
    try:
        rows = catalog_rows(REPO)
        require(not args.verifier or rows, "reference catalog is empty: no engine replay was performed")
        for row in rows:
            directory = REPO / "routines" / row["directory"]
            ref = read_json(directory / "reference.json")
            item = {"id": row["id"], "static_checks": "passed", "engine_replay": "not-run"}
            if args.verifier:
                item["authored"] = replay(args.verifier, directory / "reference.cxf.jsonld", read_json(directory / "vectors.json"), ref)
                item["matrix"] = replay(args.verifier, directory / "reference.cxf.jsonld", matrix_vectors(row), ref)
                item["engine_replay"] = "passed"
            report["routines"].append(item)
        print(json.dumps(report, indent=2))
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    except (InvalidReference, OSError, ValueError, subprocess.TimeoutExpired) as exc:
        print(str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
