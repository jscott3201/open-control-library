# G36 / CXF review — first pass, 8 September 2026

## Findings that shaped this pass

**A library integration gap, not a missing pile of JSON.** The fault library has
137 executable bundles, while the routine catalogs started this pass empty.
`tools/lint/routines.py`, `tools/lint/routine_schemas.py`, and the Rust verifier
intentionally enforced that boundary. Simply copying engine fixtures under a
routine directory would have bypassed the intended class, interface, source,
semantics, and deployment distinctions. The change here opens a narrow,
registry-backed **reference** class category and leaves deployment closed.

**The source compiler is not a body translator yet.** The current
`tools/routine-compiler/src/compiler_pipeline.rs` pipeline resolves declarations,
projects scalar ABI/source claims, allocates names, and checks declaration/interface
binding. Its TrimAndRespond integration test is useful source/declaration evidence,
not general equation/connection-to-CXF compilation. The new independent graph
author does not pretend to complete that pipeline.

**Edition is part of the behavioral contract.** The attachment is Guideline
36-2018. `scope.json`, `coverage.json`, and the established G36 source workflow
identify a 2021 Section 5 profile. ASHRAE also lists a 2024 edition. We did not
silently substitute a newer edition or retroactively attach the 2018 evidence to
2021 planning rows. Both reference cards name exact 2018 clauses, printed/PDF
page numbers, and the fact that no addenda were applied.

**Existing source-derived fixtures are not automatically interchangeable.** The
engine development fixture `thermal_zones_zone_states.jsonld` includes positive
loop hysteresis and competing-loop behavior. The attached 2018 section 5.3.5
instead defines heating and cooling with one nonzero loop and the other zero;
otherwise the state is deadband. This pass implements that literal reference on
admitted nonnegative inputs and separately exposes both-active loops. OBC explains
why field/simulation implementations add hysteresis to hard switches. That context
supports a future reviewed implementation profile, not silently changing the source
or declaring the existing engine fixture defective.

**Control data admission is not fault eligibility.** The engine's host contract
places input quality and freshness outside the status-blind graph. HostTick gives
one state update per host tick rather than Modelica event iteration. A numeric
setpoint graph returning a value is not proof that every input was fresh, coherently
sampled, correctly bound, or safe to actuate. The new offline runner requires a
complete initial frame, checks each scheduled frame, and refuses malformed values
before the real engine replay. It is not a replacement production host. Choosing
algebraic subsequences avoids claiming timer/restart/event behavior in this pass.

**Zero executable tests was reported too optimistically.** The old `--routines`
success message said all generated deployment scenarios passed even with no
rows. It now says the inventory is empty and no deployment scenarios were tested.
The reference runner independently rejects empty scenarios, missing output
assertions, off-clock windows, overlaps/gaps, missing engine samples, and bad
observed output values. That closes a common route to reassuring but vacuous
verification evidence without changing existing fault-vector semantics.

## What the two classes do

`G36-05-03-ZONE-STATE` is an algebraic classifier, not a PI controller. Its two
inputs are normalized heating/cooling loop fractions. Three mutually exclusive
Boolean state outputs implement section 5.3.5; a fourth Boolean exposes the
both-active-loop condition without converting it into a standards-defined alarm.
Strict zero tests are expressed with zero-hysteresis GreaterThreshold plus Boolean
logic, under a nonnegative input contract. Tiny positive values remain positive.

`G36-05-05-COOLING-AIRFLOW-SETPOINT` implements Table 5.5.4 and section 5.5.5.
It selects active limits by group mode, maps cooling demand, and inhibits the
mapping when AHU supply temperature is strictly above zone temperature. Both
the table on PDF page 35 and the diagram on page 36 were inspected. Equal
temperatures retain the mapping. Heat/deadband chooses the minimum. Warmup,
setback, and unoccupied limits are zero for this cooling-only table; that must
not be generalized to other terminal types or safety logic.

The airflow class receives **already calculated Vmin*** and a configured cooling
maximum as software inputs. No fixed flow configuration is baked into a canonical
identity or graph. Its explicit six-mode integer ABI is local to this reference;
it is not claimed to match Buildings' mode constants. The boundary uses finite
SI values: fraction 0..1, volume flow m3/s, and absolute temperature K.

The graphs omit generic thermal/ventilation controllers, damper PI loops,
anti-windup/neutral restart/slew limits, time-averaged ventilation, alarms,
commissioning overrides, reset requests, equipment proofs, interlocks, and live
writes. These omissions are visible in the cards, not buried in an implementation
note. Both routines remain absent from generated deployment inventory.

## CXF decisions

OBC distinguishes parameterized CDL library logic from a specifically configured
CXF representation. This pass keeps canonical class/interface identity separate
from reference export data. The two reference classes have no structural choices;
their empty specializations are meaningful and their operating software values
are runtime inputs. They are not a return to fixed-value variant catalogs.

The graphs use the engine-compatible flat composite shape, typed scalar ports,
explicit child ownership, and elementary algebraic CDL blocks. `hasInstance`
associations were added to the new exports alongside the existing typed ownership
properties. No existing fault graph was rewritten.

The prose CXF property table currently names `connectedTo`, while the upstream
`modelica-json` extractor and its S231 property vocabulary use `isConnectedTo`.
The existing engine/library dialect also uses `isConnectedTo`; this pass retains
it rather than blindly replacing the predicate from the prose table. Universal
ontology/serialization interoperability is not claimed. A later export review
should exercise the chosen official converter/core version as an independent
interoperability check, including instance IRIs, class vocabulary, parameter
metadata, units, enumeration representation, and round trips.

## Catalog, cards, and evidence

The v3 canonical registry has two explicit `reference` rows. Each bundle reuses
class-manifest v1, interface v3, and specialization v1, then adds a compact
reference scalar/ABI contract, source notes, CXF, vectors, and two SVG views.
Only registry-backed, fully validated reference directories are exempt from the
old fixture-placement rejection. Synthetic schema fixtures, unregistered bundles,
and generated deployment rows are not promoted.

The book now has a distinct **Control Routines** section. Cards use the existing
fault-card visual language—metadata, interface table, readable behavior,
assumptions, diagrams, and downloadable artifacts—without importing fault severity,
confidence, or a misleading deployment badge. The overview SVGs are readable
functional views; the full diagrams show the actual emitted block connections.

Software evidence obtained during this pass: 21 authored reference scenarios and
340 generated boundary-matrix cases passed through the actual existing engine
verifier, covering 403 sampled frames and 1,612 output assertions. The unchanged
137 fault bundles also passed all 1,760 existing scenarios. Negative unit tests
cover malformed frames, enum/type/range refusals, clocks/schedules, missing initial
values, dynamic ordering failures, no-vacuous evidence, and graph/trace damage.
The generated book and SVG fitment checks also ran locally. Full source/compiler
and pinned semantic dependency gates run through the repository CI; inspect that
run separately from the local replay report.

No independent Modelica differential execution, closed-loop building simulation,
hardware-in-the-loop test, field functional test, or commissioning took place.
No whole-G36 compliance or safety certification is implied. All legacy fault tests
passing is regression evidence, not an independent re-audit of every fault formula.

## Next review decisions

Resolve the intended production source edition with Justin, then take one temporal
workstream: cooling-only reset requests plus setpoint-change suppression, followed
by their trim-and-respond consumer. Do not expand to full reheat/ventilation/AHU
control until the host lifecycle, input admission, commissioning paths, and safety
composition are explicit and tested. G36 section 5.1.1 allows alternative underlying
logic with equivalent functional performance; equivalent performance still needs
evidence, not a renamed fixture or a passing schema.

## Sources and inspection locations

Primary supplied sequence source: **ASHRAE Guideline 36-2018**, section 5.3.5
(printed p31 / PDF p33), Table 5.5.4 and section 5.5.5 (printed p33 / PDF p35),
Figure 5.5.5 (printed p34 / PDF p36). General obligations were reviewed in sections
5.1.1–5.1.11. The copyrighted PDF is not part of this repository change.

External primary references, read 8 September 2026:

- [OBC CXF specification](https://obc.lbl.gov/specification/cxf.html), especially
  configured logic, typed connectors, ownership, and instance representation.
- [OBC overview](https://obc.lbl.gov/specification/index.html) and
  [verification](https://obc.lbl.gov/specification/verification.html), which
  distinguish control-logic verification from system validation/commissioning.
- [OBC implementation discussion](https://obc.lbl.gov/specification/example.html),
  including hard-switch hysteresis and stated differences from Guideline 36.
- [Official modelica-json extractor](https://github.com/lbl-srg/modelica-json/blob/master/lib/cxfExtractor.js)
  and [S231 vocabulary](https://github.com/lbl-srg/modelica-json/blob/master/lib/s231ClassesProperties.ttl)
  for `isConnectedTo`.
- [ASHRAE read-only edition listing](https://www.ashrae.org/technical-resources/standards-and-guidelines/read-only-versions-of-ashrae-standards).

Repository inspection: Library routine lint/schema/compiler/verifier/book code;
engine `docs/product-contract.md`, `docs/execution-profile.md`,
`docs/host-responsibilities.md`, comparator/switch implementations, and G36
fixtures; Studio's current read-only 1.0 product boundary. Downstream repositories
were read for alignment but not modified.
