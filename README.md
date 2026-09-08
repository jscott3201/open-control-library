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

Each rule is one folder of four files that travel together:

| File | What it is |
|---|---|
| `card.md` | the fault card — what the rule detects, the points and parameters it needs, likely diagnoses, energy impact, and every judgment call written down |
| `rule.cxf.jsonld` | the detection logic itself, as elementary CDL blocks |
| `vectors.json` | executable test scenarios that pin the rule's behavior from both sides of every threshold |
| `diagram.svg` | the block graph, drawn |

A rule is marked **verified** only when the engine at a pinned revision
replays every vector green. The engine's `content_id` is stamped on the card,
so the verified logic stays identifiable forever.

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
- **CI verification** (`tools/verify`) — every push replays every rule
  against the pinned engine.

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

# run one rule's vectors against the engine (sibling checkout of open-control required)
cargo run --manifest-path tools/verify/Cargo.toml -- faults/ahu/AHU-0016

# run everything
cargo run --manifest-path tools/verify/Cargo.toml -- --all

# build the book locally
python3 tools/book/generate.py && mdbook serve book --open
```

## Layout

- **`SCHEMA.md`** — the normative contracts. Read this first.
- **`faults/<equip>/<FAULT-ID>/`** — one folder per rule; `faults/registry.json` maps them all.
- **`points/`** — canonical point dictionaries.
- **`playbooks/`**, **`clusters/`** — remediation workflows and fault syndromes.
- **`tools/`** — the verifier, lints, and book generator.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any additional
terms or conditions.
