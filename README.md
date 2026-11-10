# open-control-library

**Executable, engine-verified HVAC fault detection rules.** Every rule is a
CDL block graph, shipped as CXF JSON-LD that the
[open-control engine](https://github.com/jscott3201/open-control-engine)
loads directly — no code generation, no black box.

📖 **[Browse the library](https://jscott3201.github.io/open-control-library/)** —
every rule, playbook, and point dictionary, published as a book.

> **Status: early access.** Shared for review ahead of a broader release;
> content and contracts may still change.

![Library architecture](assets/architecture.svg)

## Why

Most FDD libraries are papers, spreadsheets, or vendor black boxes. This one
runs — and you can audit every step, from the plain-language fault card down
to the exact logic the engine verified.

## How a rule ships

A rule ships as a **bundle**: one folder whose files travel together.
`SCHEMA.md` is the contract for every one of them.

| File | What it is |
|---|---|
| `card.md` | the fault card — what the rule detects, the points and parameters it needs, likely diagnoses, energy impact, and every judgment call written down |
| `graph.oce` | the detection logic itself, as elementary CDL blocks, written in the engine's authoring text form; this is the source of truth |
| `graph.cxf.jsonld` | the same logic as CXF JSON-LD, emitted from `graph.oce` by the pinned `oce` tool and never edited by hand; this is what a host loads |
| `vectors.json` | executable test scenarios that pin the rule's behavior, compared exactly |
| `mutants/` | copies of the logic with one deliberate mistake each, which the scenarios must reject |
| `receipt.json` | the engine's own record of the run: which graph, which vectors, which engine version, every result |
| `diagram.svg` | the block graph, drawn |

**The corpus is not converted to this layout yet** (issues #107 and #108).
Today a rule's folder holds `card.md`, `rule.cxf.jsonld` (hand-written CXF),
`vectors.json` and `diagram.svg`, no folder has a mutant or a receipt, and
"verified" on a card means the earlier bar: the engine at `ENGINE_PIN`
replayed every vector green.

Once a bundle is converted, it is **verified** when the `oce` tool built from
the engine commit in `ENGINE_PIN` has replayed every scenario green and every
mutant has failed at least one. Its card then carries a stamp copied from the
receipt (the graph's content id, the engine version, the vector set's id and
the mutants caught), and CI regenerates the receipt on every pull request.
Verified is not deployment qualification and not a conformance claim to any
standard; `SCHEMA.md` says exactly what it does and does not claim.

Here's a real one — AHU-0016, simultaneous heating and cooling:

![AHU-0016 block graph](faults/ahu/AHU-0016/diagram.svg)

## What's inside

- **137 verified fault rules** across fourteen equipment families — air handlers,
  VAV boxes, fan-powered terminals, rooftop units, heat pumps, chillers, cooling
  towers, hot-water plants, hydronic heat exchangers, fan coils, energy recovery
  ventilators, pumps, VFDs, and cross-equipment sensor-health rules. The
  [Fault Code Map](https://jscott3201.github.io/open-control-library/registry.html)
  lists them all.
- **Point dictionaries** (`points/`) — every canonical point name grounded in
  Brick 1.4.4 and ASHRAE 223P, so binding a rule to a real building is
  mechanical.
- **Remediation playbooks** (`playbooks/`) — verify → remote fix → on-site
  service → confirm, with typical costs and times.
- **Fault clusters** (`clusters/`) — faults that share a root cause, with a
  trigger rule and a fix order.
- **CI verification** (`tools/verify`) — every pull request replays every rule
  against the engine commit in `ENGINE_PIN`, and the same gate runs again after
  each merge and once a day. One required status, `CI OK`, summarizes it.

Every card cites its sources — standards, research reports, and this
library's own simulation studies — and records each place the rule departs
from them. The detail lives on the cards; start with any rule in the book.

## G36 control routines

The [routine catalog](routines/README.md) now includes two independently authored
**G36-2018 reference subsequences**: thermal-zone state classification and
cooling-only VAV airflow setpoints. Each has a plain-language card, typed
interface, source notes, CXF graph, diagrams, and executable vectors. The generated
book gives them their own **Control Routines** section rather than presenting
them as fault rules.

These are reviewable, engine-replayed **parts of a sequence**, not a complete
terminal controller or a qualified deployment. The deployment inventory remains
empty. The 2021 Section 5 planning/source work continues separately; the 2018
reference evidence does not satisfy its coverage claims. See
[routine replay and limits](routines/README.md#reference-replay) and
[the first-pass handoff](Handoff.md).

## Design stance

The graph computes *fault-given-valid-data*, and only that. Data quality,
operating-state gating, and suppression are host concerns, declared on the
card for the host to enforce — never buried in the logic. Rules stay
portable, and every judgment call is written down where a reviewer can
disagree with it.

## Quick start

```sh
# read the contracts
$EDITOR SCHEMA.md

# fetch the pinned engine and Modelica Buildings sources into sibling directories
bash .github/scripts/pinned-sources.sh fetch

# run the whole gate: lints, tests, every rule replayed on the pinned engine, the book
bash .agents/gate.sh

# run one rule's vectors against the engine
cargo run --locked --manifest-path tools/verify/Cargo.toml -- faults/ahu/AHU-0016
```

`.agents/gate.sh` is the single source of the gate commands; CI mirrors it.
The issue-to-merge process, the standing rules and what CI runs when are in
`AGENTS.md`. To preview the book locally, see `book/README.md`.

## Layout

- **`SCHEMA.md`** — the bundle contract and every other file format. Read this first.
- **`AGENTS.md`** — how work lands here: process, standing rules, CI and the gate.
- **`faults/<equip>/<FAULT-ID>/`** — one bundle per fault rule; `faults/registry.json` maps them all.
- **`routines/`** — the routine catalog: two Guideline 36 reference bundles, and the pins, schemas and inventories around them.
- **`points/`** — canonical point dictionaries.
- **`playbooks/`**, **`clusters/`** — remediation workflows and fault syndromes.
- **`ENGINE_PIN`** — the one engine commit every verified stamp refers to.
- **`tools/`** — the lints and the book generator, and the inherited verifier that the pinned `oce` tool replaces (issue #109).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any additional
terms or conditions.
