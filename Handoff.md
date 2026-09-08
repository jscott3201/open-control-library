# G36 routine integration — pass 1 handoff

## What is ready to review

Two **independent G36-2018 reference subsequences** now have canonical catalog
entries, typed interfaces, source notes, CXF graphs, authored vectors, an offline
input contract, overview/full-block diagrams, and book cards:

- `G36-05-03-ZONE-STATE`: section 5.3.5, classification of existing zone loops.
- `G36-05-05-COOLING-AIRFLOW-SETPOINT`: Table 5.5.4 and section 5.5.5, cooling-only
  mode limits and airflow mapping, including warm-supply inhibition.

Start at [`routines/README.md`](routines/README.md) and the cards linked there.
The wider review is in [`routines/g36/PASS1-REVIEW.md`](routines/g36/PASS1-REVIEW.md).
No fault graph was changed. The existing 2021 scope, source inventory, ontology
contracts, and deployment inventory were not repurposed or relabeled.

## Boundaries to preserve

These are real, replayable CXF graphs, **not deployable controllers**. The canonical
registry now has review rows; `generated-registry.json` remains empty. Do not
promote a reference status, rename the graph to imply deployment, or treat an
empty `--routines` check as an execution test. The verifier message was corrected
to say explicitly that no deployment scenarios were tested.

The supplied ASHRAE document is **2018**, whereas the established planning/source
profile is **2021**. These classes are independently authored from the cited 2018
clauses, not copied from engine fixtures and not translated through the unfinished
Modelica compiler. Decide the intended production edition with Justin before
porting behavior or making any 2021/2024 coverage claim. Do not commit the PDF.

The source's both-active loop case is deadband. `loop_conflict` is our separate
diagnostic, not a new G36 alarm. The engine's existing hysteretic zone-state
fixture is a different profile, not automatically a bug. Supply/zone temperature
equality does not inhibit cooling in this literal reference. Changing either
boundary requires a reviewed behavioral change and new tests.

Airflow engineering values stay runtime software inputs. `Vmin*` is calculated
upstream, not by this class. Fractions are 0..1, flows m3/s, and temperatures K.
The six mode integers are the **local reference ABI**, not Modelica Buildings'
mode codes. Translate symbols explicitly. Missing/invalid values must never be
silently replaced with an invented universal safe zero.

Offline admission protects these tests only. A live host still needs quality,
freshness, coherent frames, accepted unit/point bindings, equipment proofs,
loop/timer lifecycle, interlocks, arbitration, watchdogs, and site-specific fallback.
Studio's read-only first-release boundary is unchanged. No device-write or live
activation path was added to either downstream repository.

## Verification and working commands

The two graphs passed actual engine replay using the existing Library engine
selection: **21 authored scenarios + 340 matrix scenarios, 403 sampled frames,
1,612 output assertions**. Separately, all **137 existing fault bundles and
1,760 fault scenarios** passed the existing engine verifier without graph edits.
These counts are software evidence, not functional commissioning or certification.
The reference unit tests also exercise input refusal, no-vacuous assertions,
malformed graphs, and incomplete/misleading engine traces. CI runs the reference
checks alongside the existing schema, source, compiler, and fault gates.

Use the repository's normal sibling `open-control` checkout and existing engine
selection. Do not change that selection merely to get these tests green.

```sh
python3 -m pip install -r tools/lint/requirements-routine-schemas.txt PyYAML==6.0.3
python3 tools/lint/routines.py
python3 tools/authoring/g36_reference.py --check
python3 -m unittest discover -s tools/routines/tests -v
python3 -m unittest discover -s tools/book/tests -v
cargo build --locked --manifest-path tools/verify/Cargo.toml
python3 tools/routines/reference.py \
  --verifier tools/verify/target/debug/cxf-verify \
  --report target/g36-reference-results.json
python3 tools/book/generate.py
```

Without `--verifier`, the reference command reports `engine_replay: not-run`.
Do not count static validation as replay. `g36_reference.py` is a small independent
block-graph author, not a Modelica body compiler. The matrix oracle expresses the
source behavior separately and calls the Rust engine for observed values.

## The next useful pass

After the source-edition decision, keep the next increment narrow: cooling-only
SAT/static-pressure reset requests and their setpoint-change suppression. That
work adds real temporal behavior and should be reviewed together with the engine's
HostTick/event semantics before implementation. Cover 95%/85% retention, threshold
equality, persistence timers, interruption/reset, suppression boundaries, source
quality loss, and restart behavior. Then connect requests to a qualified trim-and-
respond profile with explicit delay, update cadence, sign, cap, and critical-zone
importance behavior. Do not leap from a passing static setpoint graph to a complete
reheat, ventilation, freeze-protection, or AHU controller.

Ask Justin when source interpretation, site behavior, or edition choice changes
the intended result. Preserve the small, readable class bundles rather than
creating many fixed-parameter variants or inventing a second deployment catalog.
