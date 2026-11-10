# open-control-library bundle contract

This file is the normative contract for every file format in this repository. A statement it makes
about a file is a requirement on that file. Where a tool enforces a requirement, the tool is named
beside it; where no tool does yet, that is said beside it.

The library holds **bundles**. A bundle is one directory that holds one piece of control logic, the
scenarios that pin its behaviour, deliberately broken copies of the logic that those scenarios must
reject, the engine's record of running all of it, and a card that tells a person what the logic is
for. Three things are true of every bundle:

- The source of truth for its logic is the **authoring text form**, `graph.oce`, written by a
  person or an agent.
- Its CXF document, `graph.cxf.jsonld`, is emitted from that text by the pinned `oce` tool and is
  never edited by hand. CXF is the exchange format of the Control Description Language (OBC §8;
  ANSI/ASHRAE Standard 231-2026 §6).
- It is **verified** when the pinned `oce` tool has replayed its vectors and every scenario passed,
  and has replayed them against each of its mutants and every mutant failed.

The **pinned `oce` tool** is the `oce` binary of the
[Open Control Engine](https://github.com/jscott3201/open-control-engine), built from the one engine
commit that `ENGINE_PIN` names. Code that loads, replays, compiles or authors a control graph
belongs to the engine. The inherited `tools/verify`, `tools/routine-compiler` and `tools/authoring`
are not part of this contract; L4 removes them.

## How to read this contract

**Revision rule.** Every format and schema this repository defines is at revision 1 and stays at
revision 1 until release 1.0.0. A format changes in place: the pull request that changes it edits
this file and rewrites every file of that format in the same change. Nothing reads an earlier
shape of any format.

**The check column.** Rules are numbered and each carries its check:

| Check entry | Meaning |
|---|---|
| a script or tool name | A step of `.agents/gate.sh` runs it on every pull request, and it enforces the rule today. `tools/verify` lint is the card and bundle lint in `tools/verify/src/lint.rs`; it reads the file names the tree has now, L2 and L3 move it onto the file names of this contract, and L4 ports it to `tools/lint/`. |
| `not yet checked (L<n>)` | No tool enforces the rule yet. The library-track step named adds the check, and what follows the colon is the check it adds. L2 is issue #107 (five bundles), L3 is #108 (every bundle), L4 is #109 (the library's own lints), L5 is #110 (the index). Until that step merges, the rule is also not yet true of every file in the tree: the corpus is brought to this contract once, in L2 and L3. |
| `reader` | No tool can enforce the rule. A reviewer does. |

**The bundle lint** is the library's own check of one bundle directory. It does not exist yet: L2
adds it under `tools/lint/` for the bundles L2 converts, and L3 and L4 extend it as the check
column says. It reads front matter, JSON and file names, and it runs no engine and parses no text
form. What it needs to know about a graph it reads from `graph.cxf.jsonld`, which rule G-3 holds
equal to the text; for the two things the CXF does not carry, the `derived_from` statement and
semantic tags, it matches a line of the canonical text.

A check that names an `oce` command gives the command and, where the engine refuses with a code,
the code. The codes are the engine's and are stable; its messages are not.

**Engine documents this contract cites.** The engine defines these formats and this contract does
not restate them. Each is a path in the engine repository, and the copy that binds a bundle is the
one at the commit in `ENGINE_PIN` (see [Open dependencies](#open-dependencies) for what that commit
holds today).

| Engine document | What it defines for a bundle |
|---|---|
| `docs/design/authoring-layer.md` | The authoring model, the text form's grammar and canonical printer, the validator's finding codes, the emitter, and `oce fmt`, `oce validate` and `oce emit`. |
| `docs/design/vectors.md` | The one vector format `oce/vectors/v1`, how a set is bound to a graph and replayed, the receipt `oce/receipt/v1`, mutant mode, every refusal code, and `oce verify`. |
| `docs/design/content-id.md` | The canonical content id, `oce:canon1:`, and exactly what it covers. |
| `docs/design/commissioning.md` | The named tolerances (`atoly`, `rtoly`, `ltoly`, `atolx`, `rtolx`, `ltolx`). |
| `docs/design/vocabulary.md` | The block vocabulary that `oce catalog` prints: what each class, port and parameter means. |
| `docs/cdl-coverage.md` | The standard's name, the citation convention used here, the block classes the engine executes, and the rule that port names are the engine's. |
| `crates/leverage/oce-cli/README.md` | Every `oce` command, its output and the exit rule: 0 yes, 1 no, 2 error. |

**Citations.** The Control Description Language is published as ANSI/ASHRAE Standard 231-2026.
A section of the OpenBuildingControl specification is written "OBC §N", and a clause of the
standard is written beside it where the engine's documents name one.

## Identifiers

Every identifier is at revision 1.

| Identifier | Where it is written |
|---|---|
| `ocl/card/v1` | `card.md` front matter, in every bundle |
| `ocl/points/v1` | `points/<family>.points.json` |
| `ocl/registry/v1` | `faults/registry.json` |
| `ocl/clusters/v1` | `clusters/clusters.json` |
| `ocl/routine-registry/v1` | `routines/registry.json` |
| `ocl/generated-routine-registry/v1` | `routines/generated-registry.json` |
| `ocl/routine-class-manifest/v1` | `class-manifest.json` in a routine bundle |
| `ocl/routine-interface/v1` | `interface.json` in a routine bundle |
| `ocl/routine-specialization/v1` | `specialization.json` in a routine bundle |
| `ocl/routine-semantic-profile/v1` | semantic profile fixtures |
| `ocl/routine-derivation-manifest/v1` | derivation manifest fixtures |
| `ocl/ontology-pins/v1` | `routines/ontology/ontology-pins.json` |
| `ocl/g36-source-inventory/v1` | `routines/g36/source-inventory.json` |
| `ocl/g36-scope/v1` | `routines/g36/scope.json` |
| `ocl/g36-coverage/v1` | `routines/g36/coverage.json` |

The engine defines three more that appear in a bundle, and this repository never defines a second
form of any of them: `oce/vectors/v1` (`vectors.json`), `oce/receipt/v1` (`receipt.json`) and the
content id tag `oce:canon1:`.

Two outputs of tools under `tools/` carry identifiers of the same family and are defined by those
tools, not here: the dataset harness result (`tools/dataset_harness/result.schema.json`) and the
trace the inherited verifier prints. Issue #109 settles what remains of each.

**Until L3 merges, write what the lints require, not what this table says.** The data files and
the lints still carry the identifiers the tree had before this contract: the prefix `cxf-library/`
in place of `ocl/`, with four of them above revision 1 (the dictionaries that have `imports`, the
routine registry, the routine interface and the coverage manifest). L3 rewrites every file and
every lint constant to the table above in one pull request. A file added before then uses the
identifier its lint accepts today.

| # | Rule | Check |
|---|---|---|
| N-1 | An identifier in the table above is spelled exactly as the table spells it, in every file that carries one. | not yet checked (L3): each lint compares the `schema` value of the files it reads |
| N-2 | No tracked data file carries an identifier of this family with a revision other than 1. | not yet checked (L3): a tracked-file pattern in the lint step |
| N-3 | The six JSON Schema resources under `routines/schemas/` have the `$id` values listed under [Routine class contracts](#routine-class-contracts), each ending in `-v1.json`. | not yet checked (L3): `tools/lint/routine_schemas.py` |

## Layout

```
open-control-library/
├── SCHEMA.md                         # this contract
├── ENGINE_PIN                        # the one engine commit the oce tool is built from
├── faults/registry.json              # one row per fault bundle
├── faults/<family>/README.md         # chapter index for one equipment family
├── faults/<family>/<ID>/             # one bundle of kind fault
├── routines/registry.json            # one row per routine bundle
├── routines/generated-registry.json  # deployment inventory, empty
├── routines/g36/<directory>/         # one bundle of kind routine
├── routines/g36/                     # source pins, source inventory, scope, coverage
├── routines/schemas/                 # six JSON Schema resources for routine class contracts
├── routines/ontology/                # ontology pin record, local vocabulary, SHACL shapes
├── points/<family>.points.json       # point dictionary per equipment family
├── playbooks/<slug>.md               # remediation playbooks
└── clusters/clusters.json            # fault clusters
```

Equipment family keys are `ahu`, `vav`, `fpb`, `rtu`, `hp`, `fcu`, `chw`, `hw`, `hx`, `erv`, `pmp`,
`vfd`, `sys` and `tower`. Each has a fault directory and a point dictionary. `zone` is a dictionary
key with no fault directory.

## A bundle

### Files

A bundle directory holds these files and no others.

```
<bundle>/
├── card.md                     # authored; its verified block is copied from receipt.json
├── graph.oce                   # authored: the logic, in the engine's text form
├── graph.cxf.jsonld            # generated from graph.oce
├── vectors.json                # authored: scenarios and expected outputs
├── mutants/<name>.oce          # authored: the logic with one deliberate mistake
├── mutants/<name>.cxf.jsonld   # generated from mutants/<name>.oce
├── receipt.json                # generated: the engine's record of the run
└── diagram.svg                 # authored: the block graph, drawn
```

| File | Origin | How it is made | What checks it | What a reader may rely on |
|---|---|---|---|---|
| `card.md` | Authored. Its `verified` block is generated. | A person writes it. The `verified` block is copied field for field from `receipt.json`. | The card rules (C). The stamp: V-5 and V-6. | What the logic is for, what it needs from a host, and which engine run stands behind it. Nothing in the card reaches the engine. |
| `graph.oce` | Authored. | Written in the engine's text form. `oce fmt --write graph.oce` makes it canonical. | `oce fmt --check` and `oce validate` (G-1, G-2). | It is the logic. Every other form of the logic in the bundle is derived from it. |
| `graph.cxf.jsonld` | Generated. | `oce emit graph.oce --namespace urn:open-control-library:bundle: -o graph.cxf.jsonld` | Byte equality with a fresh emit (G-3). `oce id` of it equals the stamp (V-6). | It is the document a host loads, and it loads with no warning on the pinned engine. Its bytes are reproducible at `ENGINE_PIN`. They change when the pin moves to another engine version, because the document names the tool version that wrote it; the content id does not change for that reason. |
| `vectors.json` | Authored. | Written in `oce/vectors/v1`. | `oce verify` reads it strictly and refuses anything the format does not define (T rules). | The scenarios, and the outputs the logic must produce in them, compared exactly. |
| `mutants/<name>.oce` | Authored. | Written as `graph.oce` is, with one mistake put in on purpose. | `oce fmt --check`, `oce validate` (M-2). | What mistake the vectors are known to catch. |
| `mutants/<name>.cxf.jsonld` | Generated. | `oce emit mutants/<name>.oce --namespace urn:open-control-library:bundle: -o mutants/<name>.cxf.jsonld` | Byte equality with a fresh emit (M-3). `oce verify` validates it as a mutant (M-5). | It is the document the engine ran as the mutant; `receipt.json` holds its digest. |
| `receipt.json` | Generated. | `oce verify vectors.json --subject graph.cxf.jsonld --require-mutants-caught --out <directory>` writes `<directory>/receipt.json`. | Byte equality with a fresh run (V-4). Its content: V-5. | The engine's own record: which graph, which vectors, which engine version, every scenario's result, every mutant's result. |
| `diagram.svg` | Authored. | Drawn by hand. | `tools/lint/svg_fitment.py`; the card links it (D rules). | A picture for a person. No tool compares it with the graph. |

A bundle of kind `routine` also holds the files under [Routine bundles](#routine-bundles).

`.agents/gate.sh` is the single runnable source of how the gate invokes these commands. The table
states what each file is made from, which is the contract; the script states how the gate runs it.

### Kinds

| `kind` | What the bundle is | Where it lives | Bundle id |
|---|---|---|---|
| `fault` | A fault detection rule: canonical points in, Boolean fault outputs out. | `faults/<family>/<ID>/` | `{FAMILY}-{NNNN}`: the family key in upper case, a hyphen, four digits |
| `routine` | A reference part of a control sequence. It is not a deployable controller. | The directory its row in `routines/registry.json` names, under `routines/g36/` | `G36-05-(01..22)-<UPPERCASE-HYPHENATED-SLUG>` |
| `sequence` | A complete control sequence for one piece of equipment. | No bundle of this kind exists. Its directory and id grammar are fixed, in this file, by the pull request that adds the first one. | not defined |

A sequence, a routine or a fault lives in this library and never in the engine repository, which
keeps only a minimal conformance corpus.

| # | Rule | Check |
|---|---|---|
| B-1 | A bundle directory holds exactly the files of [Files](#files), plus, for kind `routine`, the files of [Routine bundles](#routine-bundles). A file this contract does not name is refused. | not yet checked (L2): the bundle lint, on the bundles L2 converts; L3 for every bundle |
| B-2 | A bundle with no mutant does not meet [the bar](#the-bar): its status is `draft`, it has no `receipt.json`, and its card has no `verified` block. A bundle of any other status holds every file of [Files](#files) and at least one mutant. Kind `routine` has no `draft` status, so a routine bundle always holds a mutant. | not yet checked (L2): the bundle lint |
| B-3 | The card's `kind` is the kind its location gives: `fault` under `faults/`, `routine` under `routines/`. | not yet checked (L2): the bundle lint |
| B-4 | No bundle holds a `reference/` directory. The name is reserved for file-backed reference series, which the vector format does not have. | not yet checked (L2): B-1 refuses it |
| B-5 | Every fault bundle has exactly one row in `faults/registry.json`, and every routine bundle exactly one row in `routines/registry.json`; neither registry has a row without a bundle. | `tools/lint/registry.py` (faults); `tools/routines/reference.py` and `tools/lint/routine_schemas.py` (routines) |
| B-6 | No bundle of kind `sequence` exists. | not yet checked (L2): B-3 admits two kinds |

## Identity

A bundle has four identities. Each answers a different question, and none stands in for another.

| Identity | Question it answers | Who mints it | Where it is written |
|---|---|---|---|
| Bundle id | Which rule or routine is this? | The author, once. | The card's `id`, the folder name of a fault, the registry row. |
| Content id | Which graph, exactly? | The engine: `oce id graph.cxf.jsonld`. | The stamp and `receipt.json`. |
| Vector set id (`spec_id`) | Which vectors, and which mutants were listed? | The engine, over the canonical form of `vectors.json`. | The stamp and `receipt.json`. |
| Receipt id | Which run? | The engine, over the receipt. | `receipt.json`. |

### The bundle id

A fault id is `{FAMILY}-{NNNN}`, numbered from `0001` within its family in the order the rules were
authored. The number carries no meaning; provenance is in the card's `source` list. A routine id
names a parameterized engineering class and is independent of source paths, revisions, fixed
parameter values, ordering and hashes.

A planned bundle that a family chapter or a card already names may hold a reserved id. A reserved
id appears in prose and in index tables marked planned, never in a registry, and the next bundle
authored in that family takes the next free number after any reservation.

### The sequence name and the namespace

The text form opens with `sequence <name>`. The name is the bundle id in lower case with every
hyphen written as an underscore: `ahu_0016` for `AHU-0016`, `g36_05_03_zone_state` for
`G36-05-03-ZONE-STATE`. A mutant has the same sequence name as its bundle.

Every bundle is emitted under one namespace, `urn:open-control-library:bundle:`. The engine writes
each instance as `{namespace}{sequence}.{local}`, so the boundary input `htg_vlv_cmd` of `AHU-0016`
is `urn:open-control-library:bundle:ahu_0016.htg_vlv_cmd`, and that is the name a host binds.

### The content id

The content id is the engine's `oce:canon1:` id: the tag and 64 lower-case hexadecimal digits.
`docs/design/content-id.md` defines it. What a reader of this library needs from that note:

- **It covers** the loaded graph: every block's name, class, class signature and bound parameter
  values, bit for bit; every connection; every boundary input and output with its name, type,
  `quantity`, `unit` and `displayUnit`; and each input's accepted range.
- **The sequence name is inside every name it hashes.** Renaming a bundle therefore changes its
  content id.
- **It does not cover** the namespace, the order in which the text form lists things, comments,
  the `derived_from` statement, semantic tags, the card, the vectors, the mutants or the diagram.
- **It is not an engine identity.** The same content id on two engine versions does not promise
  the same behaviour, which is why the stamp carries the engine version beside it. A new engine
  build that changes the signature of a class the graph uses changes the content id with no edit
  in this repository.

A content id that changes is a rule change and is listed in `CHANGELOG.md` by bundle id. When the
cause is an `ENGINE_PIN` advance and not an edit, the entry says so.

### The stamp

The stamp is the `verified` block of the card. It states which graph was verified, against which
vectors, by which engine version, and how many mutants the vectors caught. Every field is copied
from `receipt.json`, and the names of the receipt fields are those of `docs/design/vectors.md`,
"The receipt".

```yaml
verified:
  content_id: "oce:canon1:<64 hex digits>"
  engine_version: "0.5.0"
  spec_id: "spec:fnv1a128:<32 hex digits>"
  mutants: { caught: 2, total: 2 }
```

| Stamp field | Receipt field it copies | Built in the engine? |
|---|---|---|
| `content_id` | header `content_id`: the graph's id as loaded | Not yet. `Engine::content_id` exists; the receipt that carries it is engine #520, and `oce id` is engine #513. |
| `engine_version` | header `engine_version`: the engine facade's version | Not yet: engine #520. |
| `spec_id` | header `spec_id`: the id of the vector set | Not yet: engine #519 mints it and engine #520 writes it into a receipt. |
| `mutants.caught`, `mutants.total` | body `mutants.caught` and `mutants.total` | Not yet: engine #521. |

The stamp carries no date and no engine commit. The date of a run is not part of what was
verified, and a stamp that held one could not be reproduced. `engine_version` is the engine's
release version and, in the engine's own words, not a build identity: the build is the commit in
`ENGINE_PIN` at the same commit of this repository, and the gate regenerates every receipt with
the tool built from it (V-4).

| # | Rule | Check |
|---|---|---|
| I-1 | A fault id matches `{FAMILY}-{NNNN}`, equals its folder name and its card's `id`, and the numbers of a family are contiguous from `0001`. | `tools/lint/registry.py` |
| I-2 | A routine id matches the routine grammar of [Kinds](#kinds), is unique, and equals its card's `id`. | `tools/routines/reference.py` |
| I-3 | A bundle id is never reused, and a bundle is not renamed without an issue that says so. | reader |
| I-4 | The `sequence` statement of `graph.oce` names the bundle id in lower case with hyphens as underscores. | not yet checked (L2): the bundle lint reads the root `@id` of `graph.cxf.jsonld`, which is the namespace followed by that name |
| I-5 | `graph.cxf.jsonld` and every mutant's CXF are emitted under `urn:open-control-library:bundle:`. | not yet checked (L2): G-3 and M-3 emit with that namespace and compare bytes |
| I-6 | A sequence name is at most 64 bytes, which the text form requires of any name. | not yet checked (L2): `oce validate`, `invalid-name` |

## The verified bar

### What verified claims

A bundle is verified when its card carries a stamp and the gate reproduces that stamp from the
bundle's own files. The claim is exactly this:

1. `graph.cxf.jsonld` is, byte for byte, what the pinned `oce` tool emits from `graph.oce`.
2. Loaded by the engine at `ENGINE_PIN`, that document has the content id in the stamp.
3. On that engine, every scenario in `vectors.json` passed: at every tick inside every window, the
   output named had exactly the value written. A Boolean and an Integer are compared for equality
   and a Real is compared bit for bit.
4. On that engine, each mutant under `mutants/` loaded, had the same inputs and outputs as the
   graph, and failed at least one scenario.
5. `receipt.json` is, byte for byte, the receipt the pinned tool writes for that run, and the stamp
   repeats its fields.

### What verified does not claim

- **It is not deployment qualification.** A verified bundle has not been commissioned, bound to a
  building's points, or run against a live controller. A routine's status stays `reference` and
  `routines/generated-registry.json` stays empty whether or not the routine is verified.
- **It is not a conformance claim to ANSI/ASHRAE Standard 231-2026.** The engine claims no
  conformance to the standard (`docs/cdl-coverage.md`), and this library adds none.
- **It is not a conformance claim to ASHRAE Guideline 36.** A card's `g36` and `standard` fields
  cite the clause an author read. No tool compares the graph with the guideline.
- **It does not say the logic is right.** It says the logic does what its vectors say, and that
  the vectors are strong enough to reject the mutants listed. A mistake that no scenario and no
  mutant exercises is not found.
- **It does not cover the host.** Data quality, operating-state gating, suppression, unit
  conversion and point binding are the host's, declared on the card and enforced by no tool here.
- **It holds for one engine version.** The stamp names it. Another version is another claim, made
  by advancing `ENGINE_PIN` and regenerating every stamp.
- **It is not field evidence.** The optional `validation` block of a fault card records simulation
  sweeps; it is a separate record and no part of the bar.

### The bar

| # | Rule | Check |
|---|---|---|
| V-1 | `oce fmt --check graph.oce` exits 0, and so does `oce validate graph.oce`. | not yet checked (L2): the two commands (G-1, G-2) |
| V-2 | The bytes `oce emit graph.oce --namespace urn:open-control-library:bundle:` writes equal `graph.cxf.jsonld`. | not yet checked (L2): the emit and a byte comparison (G-3) |
| V-3 | `oce verify vectors.json --subject graph.cxf.jsonld --require-mutants-caught` exits 0. Exit 1 means a scenario failed, the set compared nothing, or a mutant survived. Exit 2 means there is no verdict: the engine refused a file, a name or a mutant, and wrote no receipt. | not yet checked (L2): the command |
| V-4 | The receipt that run writes equals `receipt.json`, byte for byte. | not yet checked (L2): `--out` into a scratch directory and a byte comparison |
| V-5 | In `receipt.json`: `schema` is `oce/receipt/v1`, `kind` is `vectors`, `comparison.mode` is `exact`, `passed` is true, `body.status` is `pass`, `body.load_warnings_total` is 0, `body.mutants.status` is `scored`, `body.mutants.total` is at least 1 and `body.mutants.survived` is 0. | not yet checked (L2): the bundle lint reads the receipt |
| V-6 | The stamp equals the receipt fields it copies, and `oce id graph.cxf.jsonld` prints the stamp's `content_id`. | not yet checked (L2): the bundle lint and `oce id` |
| V-7 | The `oce` binary is built from the engine checkout at the commit in `ENGINE_PIN`, with no tracked file of that checkout modified, and with the Rust toolchain that checkout names. | `bash .github/scripts/pinned-sources.sh check` proves the checkout today; not yet checked (L2): the step that builds `oce` from it |
| V-8 | A status of `verified` or `reference` requires a stamp, and a status of `draft` forbids one. A bundle is never stamped on the strength of scenarios alone: a set that lists no mutant produces no mutant score to copy. | `tools/verify` lint requires, for `verified`, the fields the cards carry today; not yet checked (L2): the bundle lint, for this stamp and both directions |
| V-9 | No expectation, vector or mutant is edited to make a failing run pass. A failing scenario is a finding and is reported. A new scenario may be added and is named in the pull request. | reader |

A run-time warning does not fail the bar. The receipt counts warnings per scenario and for the set
(`body.totals.warnings`), where a reader can see them. A load warning cannot occur under V-1,
because `oce validate` reports every engine warning as an error; V-5 checks it again in the
receipt.

### Why a vacuous PASS cannot meet the bar

A vacuous PASS is a green result that compared nothing, or nothing that could have failed.

| What would make a PASS vacuous | The rule that refuses it | Check |
|---|---|---|
| A set with no scenario, or a scenario with no expectation. | The set's verdict is `no_data` and `oce verify` exits 1. | not yet checked (L2): V-3 |
| A window that contains no tick, runs backwards, or lies outside the clock. | The engine refuses the file. | not yet checked (L2): V-3, codes `window-uncovered`, `invalid-window`, `window-outside-clock` |
| An input step placed after the horizon, where it could never take effect, or a horizon the clock never reaches. | T-9; the engine refuses the file. | not yet checked (L2): V-3, codes `step-after-horizon`, `horizon-off-cadence` |
| Two scenarios with one name, so that one hides the other in a report. | The engine refuses the file. | not yet checked (L2): V-3, code `duplicate-scenario` |
| A field the format does not define, such as `tolerance`, silently ignored. | The format is closed at every level. | not yet checked (L2): V-3, code `unknown-key` |
| A scenario that leaves an input without a value, so the graph runs on a default. | The engine refuses the set. | not yet checked (L2): V-3, code `input-not-supplied` |
| A boundary output that no scenario asserts. | T-4. | not yet checked (L3): the bundle lint |
| Vectors so weak that a broken graph passes them. | A mutant survives: V-3 and V-5. | not yet checked (L2) |
| No mutant at all. | V-5: `body.mutants.total` is at least 1. | not yet checked (L2) |
| A mutant that is the graph under another name, or that could never have run. | The engine refuses a mutant that does not load, whose inputs or outputs differ from the graph's, or whose content id equals the graph's. A mutant that only renames a block runs, passes every scenario and survives. | not yet checked (L2): V-3, codes `mutant-load-failed`, `mutant-boundary-differs`, `mutant-identical` |

### Why a wrong PASS cannot meet the bar

A wrong PASS is a green result that belongs to something other than the bundle's files.

| What would make a PASS wrong | The rule that refuses it | Check |
|---|---|---|
| The CXF was edited by hand and no longer matches the text form. | V-2. | not yet checked (L2) |
| The receipt is for another graph. | V-6: the receipt's `content_id` equals `oce id` of the committed CXF. | not yet checked (L2) |
| The receipt is for other vectors, or for a list of mutants that changed since. | V-4: the regenerated receipt carries the `spec_id` of the committed `vectors.json`, which covers the mutant paths, and the digest of each mutant's bytes. | not yet checked (L2) |
| The receipt is stale: the files changed after it was written. | V-4 regenerates it on every pull request. | not yet checked (L2) |
| The run used another engine. | V-7, and the receipt's `engine_version` is inside the bytes V-4 compares. | not yet checked (L2) |
| A scenario passed under edited parameters, on a graph that is not the stamped one. | T-3: a bundle's vectors carry no `params`. | not yet checked (L3): the bundle lint |
| A Real output passed inside a band nobody wrote down. | The format has no tolerance field; a Real `equals` is exact. | not yet checked (L2): V-3 |
| An expectation was loosened until the run went green. | V-9. | reader |

### `ENGINE_PIN`

`ENGINE_PIN` holds one lower-case 40-digit hexadecimal engine commit and a line feed. It is the
only pin of the engine. The `oce` tool is built from that commit; nothing is downloaded as a built
artifact. An advance of the pin regenerates every `graph.cxf.jsonld`, every mutant's CXF and every
`receipt.json`, re-stamps every card, and lists in `CHANGELOG.md` every bundle whose content id
moved. The pin advances only under an issue that says so.

| # | Rule | Check |
|---|---|---|
| E-1 | `ENGINE_PIN` holds exactly one lower-case 40-digit hexadecimal commit and a line feed. | `bash .github/scripts/pinned-sources.sh check` |
| E-2 | Every stamp's `engine_version` is the version of the engine at `ENGINE_PIN`. | not yet checked (L2): V-4 implies it |
| E-3 | A pull request that changes `ENGINE_PIN` leaves no generated file stale. | not yet checked (L2): V-2 and V-4 on every bundle the gate covers |

## The graph: `graph.oce` and `graph.cxf.jsonld`

### Design stance

- **A fault rule is one flat composite block.** Canonical point inputs feed elementary comparison,
  logic and timing blocks, which drive Boolean fault outputs. The text form writes exactly this
  shape: one root, leaf blocks, connections.
- **The graph computes the fault condition given valid data, and only that.** Data quality,
  operating-state gating, suppression and energy accumulation are the host's, declared in the card
  and never encoded in the graph. The engine does not read status; a host enforces `preconditions`
  and `operating_states` before it trusts `yFault`.
- **Semantics live in the point dictionary,** keyed by canonical point name. A graph carries no
  Brick, ASHRAE 223 or QUDT term.

### Rules

| # | Rule | Check |
|---|---|---|
| G-1 | `graph.oce` is canonical text: `oce fmt --check graph.oce` exits 0. | not yet checked (L2): the command |
| G-2 | `graph.oce` is valid on both validator stages: `oce validate graph.oce` exits 0. This covers every name, class, parameter, port and connection, single assignment, algebraic loops, and every engine warning, each of which is an error finding. | not yet checked (L2): the command |
| G-3 | `graph.cxf.jsonld` equals, byte for byte, what `oce emit graph.oce --namespace urn:open-control-library:bundle:` writes. It is never edited. | not yet checked (L2): the emit and a byte comparison |
| G-4 | The `derived_from` statement of `graph.oce` equals the card's `derived_from`: `derived_from none` where the card has `null`, otherwise the same id. | not yet checked (L2): the bundle lint |
| G-5 | A block port is named as `oce catalog <class>` prints it at `ENGINE_PIN`. This contract fixes no port spelling. | not yet checked (L2): G-2, `unknown-port` |
| G-6 | Every block class is one the engine's catalog lists as authorable. | not yet checked (L2): G-2, `class-not-found`, `class-not-authorable` |
| G-7 | Kind `fault`: the boundary inputs of the graph are exactly the card's `points`, with the same bare names, and the boundary outputs are exactly the names in the card's `outputs`. | `tools/verify` lint |
| G-8 | Kind `fault`: every boundary output is Boolean, and one of them is named `yFault`. | not yet checked (L3): the bundle lint |
| G-9 | Kind `fault`: a boundary declaration carries no tag. Units are those of the point dictionary, the host feeds them, and the graph converts none. | not yet checked (L3): the bundle lint |
| G-10 | Kind `fault`: a block assigns a parameter only when the value differs from the class default, or when a card `params` entry names that parameter. | not yet checked (L4): the bundle lint, against the defaults `oce catalog --json` prints |
| G-11 | No boundary declaration of any kind carries `@brick`, `@s223`, `@qudt` or `@haystack`. | not yet checked (L3): the bundle lint |
| G-12 | No graph has a conditional component, a nested composite or an array connector. The text form cannot write one. | not yet checked (L2): G-2 refuses what the grammar does not have |

`yFault` is true while the fault condition persists. A fault graph may have further Boolean
outputs, each listed in the card's `outputs`, and they are of two kinds that the card's
description of each output states:

- **An evaluability flag** (`y…Ok`). When the reference semantics include a test that can be
  computed from the rule's own inputs and that decides whether the rule can be evaluated at all,
  the graph exposes it as an output. False means the rule cannot be evaluated, and the host
  consults it before it reads `yFault`.
- **A sub-condition or direction flag.** Diagnostic detail only. False never means the rule cannot
  be evaluated.

A host does not treat every output other than `yFault` as an evaluability gate. Which kind an
output is, and whether a rule needs an evaluability flag, is a `reader` rule: no tool decides it.

The library sets `delayOnInit = true` on every `CDL.Logical.TrueDelay`, against the class default,
so that a condition already true at start-up still persists for the full delay. The emitter never
fills a value the author did not write; the text form writes this one. This is a `reader` rule, and
the engine's vocabulary records it as a convention note on the class.

## Vectors: `vectors.json`

`vectors.json` is one document in the engine's vector format, `oce/vectors/v1`. The format, its
clock, its inputs, its windows, how names resolve, how values are typed and compared, its bounds
and every refusal code are defined in `docs/design/vectors.md` and nowhere in this repository.
This section holds only the rules this library adds for its own bundles.

| # | Rule | Check |
|---|---|---|
| T-1 | `vectors.json` loads as `oce/vectors/v1` and binds to `graph.cxf.jsonld`. | not yet checked (L2): `oce verify` exits 0 or 1, never 2 |
| T-2 | An input and an output are named bare, as `graph.oce` declares them, and never by a full path. | not yet checked (L3): the bundle lint |
| T-3 | `vectors.json` has no `params` map, at the set level or in a scenario. Every scenario runs the stamped graph with the parameter values `graph.oce` writes. | not yet checked (L3): the bundle lint |
| T-4 | Every boundary output of the graph is asserted by at least one window of at least one scenario. | not yet checked (L3): the bundle lint |
| T-5 | The `mutants` list names exactly the files `mutants/<name>.cxf.jsonld` that the bundle holds: no listed path is missing and no file is unlisted. | not yet checked (L2): the bundle lint; a missing file is `oce verify` exit 2 |
| T-6 | A Real expectation is an exact literal. A bundle that needs a band waits for the format's `within`, which uses the named tolerances of `docs/design/commissioning.md`; until the engine has it, the key is refused. | not yet checked (L2): `oce verify`, `unknown-key` |
| T-7 | The scenarios of a fault bundle include the published test vectors of the reference its card cites, at least one case at a threshold edge, and, where the graph has timing state, at least one transient that exercises the delay and its reset. | reader; M-7 with M-6 is the mechanical half |
| T-8 | A window does not assert the tick at which a delay expires. It leaves at least one step of margin on each side of that edge. | reader |
| T-9 | No step of an input's step list lies after the horizon. Such a step never takes effect, so a file that has one says less than it appears to, and it is refused. | not yet checked (L2): `oce verify`, `step-after-horizon`. The inherited verifier does not refuse it; issue #120 adds that refusal and L3 corrects the files that have one |

## Mutants

A mutant is the bundle's graph with one mistake put in on purpose. It exists to show that the
vectors can fail. `docs/design/vectors.md`, "Mutant mode", defines how the engine validates a
mutant, runs it and scores it.

**What makes a good mutant.** One plausible authoring mistake, of the kind a person or an agent
makes when writing the graph: a comparison flipped or made inclusive, a threshold moved by the
margin the vectors leave around it, a delay dropped or shortened, two inputs wired the wrong way
round. The mutant keeps every input and output of the graph and loads cleanly. A mutant that does
not load, that changes an input or an output, or that differs from the graph only in a name proves
nothing: the engine refuses the first two, and the third survives and fails the bar.

A gap the vectors are known to leave is written in the card's `## Deviations`. It is not shipped
as a mutant that survives.

| # | Rule | Check |
|---|---|---|
| M-1 | A mutant's name matches `[a-z][a-z0-9_]*`. Its two files are `mutants/<name>.oce` and `mutants/<name>.cxf.jsonld`. | not yet checked (L2): the bundle lint |
| M-2 | `mutants/<name>.oce` is canonical and valid, as G-1 and G-2 require of the graph, and has the graph's sequence name. | not yet checked (L2): `oce fmt --check`, `oce validate`, the bundle lint |
| M-3 | `mutants/<name>.cxf.jsonld` equals, byte for byte, what `oce emit mutants/<name>.oce --namespace urn:open-control-library:bundle:` writes. | not yet checked (L2): the emit and a byte comparison |
| M-4 | Each entry of the `mutants` list in `vectors.json` has a `description` that states the mistake in one sentence. | not yet checked (L2): the bundle lint requires a non-empty string; what it says is `reader` |
| M-5 | Every mutant is valid: it loads, its inputs and outputs equal the graph's in path and type, and its content id differs from the graph's. | not yet checked (L2): V-3, exit 2 with `mutant-load-failed`, `mutant-boundary-differs` or `mutant-identical` |
| M-6 | Every mutant is caught: at least one scenario fails on it. | not yet checked (L2): V-3 and V-5 |
| M-7 | A bundle whose graph compares a signal with a threshold ships a mutant that changes the strictness of one such comparison or moves its threshold. A bundle whose graph has a delay ships a mutant that removes or shortens one. | reader |
| M-8 | A mutant is one mistake. It is not a rewrite of the graph. | reader |

## The card: `card.md`

A card is YAML front matter followed by Markdown prose. The front matter is closed: a key this
section does not name for the card's kind is refused.

### Front matter of every kind

| Field | Type | Meaning |
|---|---|---|
| `schema` | string | `ocl/card/v1` |
| `kind` | enum | `fault` \| `routine` \| `sequence` |
| `id` | string | The bundle id |
| `name` | string | A short name for a person |
| `equipment` | string | An equipment family key: the stem of one file `points/<equipment>.points.json` |
| `status` | enum | Kind `fault`: `draft` \| `verified`. Kind `routine`: `reference`. |
| `g36` | string or null | The clause of ASHRAE Guideline 36 the bundle implements, or null. Never null for kind `routine`. |
| `derived_from` | string or null | The id of the bundle this one is derived from, or null. It equals the `derived_from` statement of `graph.oce`. It is an opaque provenance id: nothing resolves it until the bundle index exists. |
| `verified` | map | [The stamp](#the-stamp). Absent while the status is `draft`. |

`status` has three values and each has a check. `draft` makes no claim: the bundle has no stamp.
`verified` means a fault bundle meets [the bar](#the-bar). `reference` is the one status of a
routine: the routine meets the bar and is not a deployable controller. There is no other value. A
status is added to this table, with the check that gives it a meaning, by the pull request that
first needs it.

### Front matter of kind `fault`

| Field | Type | Req | Meaning |
|---|---|---|---|
| `phase` | int | ✓ | Rollout phase, 1 to 4 |
| `method` | enum | ✓ | `rule` \| `statistical` \| `ml` \| `meta` |
| `severity` | int | ✓ | 1 critical, 2 high, 3 warning, 4 info |
| `category` | enum | ✓ | `CRITICAL_WASTE` \| `EFFICIENCY_LOSS` \| `EXCESS_CONSUMPTION` \| `COMFORT_ENERGY` \| `PROTECTIVE` |
| `confidence` | enum | ✓ | `HIGH` \| `MEDIUM` \| `LOW`: the strength of the evidence |
| `estimation_method` | enum | ✓ | `DIRECT_MEASUREMENT` \| `BASELINE_COMPARISON` \| `PROXY_ESTIMATION` \| `QUALITATIVE_ONLY` |
| `source` | list | ✓ | Provenance references: a reference section, a report, a guideline clause |
| `playbooks` | list | ✓ | Playbook slugs under `playbooks/` |
| `operating_states` | string | ✓ | The states the rule applies in: `all`, or a list in prose |
| `preconditions` | string | ✓ | The gate a host enforces before it evaluates the rule, in prose |
| `points` | list | ✓ | The bare names of the points the rule consumes |
| `outputs` | list | ✓ | `{name, description}` for each boundary output |
| `params` | map | ✓ | name → `{default, unit, description, cxf}` |
| `energy_impact` | map | ✓ | `{affected_subsystem, savings_range, climate_sensitivity, runtime_estimation}` |
| `emissions` | map | ✓ | `{scope, method}` |
| `clusters` | list | | Ids of the clusters the rule belongs to |
| `suppresses` | list | | Ids of rules silenced while this fault is active |
| `suppressed_by` | list | | Ids of rules that silence this one while active |
| `related` | list | | Rules that tend to occur with this one; informational |
| `adjudicates` | map | | Sensor-health rules only: `{points: [...], verdict: invalid_while_active \| ambiguous}` |
| `validation` | list | | Records of simulation sweeps; see below |

- **`points`.** Each entry is a bare name that resolves in `points/<equipment>.points.json`, as a
  concrete point of that dictionary or as one of its aliases. The graph's boundary input keeps that
  exact bare name.
- **`params.<name>.cxf`** is the path of the block parameter the card parameter sets, written
  `<block>.<parameter>` with the block name and the parameter name `graph.oce` uses
  (`persist.delayTime`). A host retunes a loaded rule by setting that path. It is a list of paths
  when one card parameter sets several block parameters, and a host then sets every path in the
  list together. `default` is the value `graph.oce` assigns at that path.
- **`g36`** is set on a fault only when the rule implements a fault condition that Guideline 36
  itself numbers. A rule that draws on the guideline in another way cites it in `source` and has
  `g36: null`.
- **`adjudicates`** names the canonical points whose validity a sensor-health rule judges. With
  `invalid_while_active`, a host treats each point as invalid for every consumer while the fault
  asserts. `ambiguous` marks a redundancy-pair rule that cannot say which member drifted. A host
  derives the set of affected rules from the `points` lists of the other cards, so the set stays
  complete as rules are added.
- **`validation`** is a list of records of empirical runs against the rule. Each entry has `kind`
  (`simulation_fpr` or `simulation_tpr`; for the second, `failures` counts missed detections),
  `harness`, `date`, `fleet` (one line: buildings, climates, period), `scenarios` and `failures`
  (counts), and an optional one-line `notes`. A result is a fact about one fleet and one gating
  configuration. A card without the block has not been swept.

### Front matter of kind `routine`

| Field | Type | Req | Meaning |
|---|---|---|---|
| `standard` | string | ✓ | The edition the author read, written in full: `ASHRAE Guideline 36-2018`. No addendum is applied unless the string names it. |
| `scope` | string | ✓ | What part of a sequence the routine is, in a few words |
| `deployment` | string | ✓ | Exactly `not qualified; not in generated deployment inventory` |
| `integer_codes` | map | | input name → `{symbol: integer}`: the integers an Integer input accepts, and what each means |
| `ordered_inputs` | list | | Pairs `[low, high]` of Real inputs with one unit, where the first may not exceed the second |

`integer_codes` and `ordered_inputs` state what a host must supply. The graph does not enforce
them and the engine does not read the card. The codes of one input are consecutive from 1. They
are this library's own encoding for the routine, distinct from the symbolic members of the
routine's interface and from the integer encoding of any other library, and a host translates
symbols to them explicitly.

### Rules

| # | Rule | Check |
|---|---|---|
| C-1 | `schema` is `ocl/card/v1`, and the front matter has exactly the keys this section names for the card's kind, with every required key present. | `tools/verify` lint checks the identifier the cards carry today; not yet checked (L3): the bundle lint, for this identifier and the closed key set |
| C-2 | `id` equals the bundle id, and for a fault the folder name. `equipment` is a dictionary key, and for a fault the family folder. | `tools/verify` lint (faults); not yet checked (L2): the bundle lint (routines) |
| C-3 | `status`, `method`, `category` and `severity` hold one of the values listed. | `tools/verify` lint, which also accepts two status values this contract does not have (`adopted`, `deprecated`; no card uses either); not yet checked (L3): the bundle lint refuses them |
| C-4 | `points` and `outputs` are not empty. Every entry of `points` resolves in the family dictionary. | `tools/verify` lint |
| C-5 | Every entry of `clusters` is an id in `clusters/clusters.json`, and every entry of `playbooks` names a file `playbooks/<slug>.md`. | `tools/verify` lint |
| C-6 | `adjudicates.verdict` is one of its two values, `adjudicates.points` is not empty, and each of its points is among the card's own `points`. | `tools/verify` lint |
| C-7 | Every path in `params.<name>.cxf` names a parameter that `graph.oce` assigns on that block. | `tools/verify` lint |
| C-8 | `params.<name>.default` equals the value `graph.oce` assigns at each path of that parameter. | not yet checked (L4): the bundle lint |
| C-9 | `derived_from` is null or an id in the engine's provenance-id grammar. | not yet checked (L2): `oce validate`, `invalid-provenance-id`, through G-4 |
| C-10 | Kind `routine`: `deployment` is the exact string above, `g36` is not null, and `standard` is not empty. | not yet checked (L2): the bundle lint |
| C-11 | Kind `routine`: each key of `integer_codes` names an Integer input of the graph, and its codes are consecutive from 1. Each pair of `ordered_inputs` names two Real inputs of the graph that declare one unit. | not yet checked (L2): the bundle lint |
| C-12 | The name, the method and the status in the bundle's registry row equal the card's. | `tools/lint/registry.py` (faults); `tools/routines/reference.py` (routines: id, name and status) |

### Prose

The prose sections of a fault card, in order: `## Description`, `## Detection Logic`,
`## Possible Diagnoses`, `## Energy Impact`, `## Emissions Impact`, `## Deviations` and an optional
`## Notes`. `## Deviations` lists every difference from the source reference and why; it is
required and may say "None". `## Detection Logic` holds the equation in a fenced block and the
diagram as `![…](diagram.svg)`.

A card is a specification, not a design journal. Description runs to about ten lines. Detection
Logic runs to about fifteen lines beyond the equation and the diagram, and covers timing,
strictness and what a deployer must know, without narrating the diagram block by block. Energy and
Emissions run to about eight lines each. Deviations keeps every engineering decision, each as one
bullet of two to five lines: the decision, one sentence of why, the citation. A card does not
narrate its scenarios, which `vectors.json` records, except for one sentence that names an engine
behaviour a scenario pins on purpose. `faults/ahu/AHU-0016/card.md` is the exemplar. The section
order and the lengths are `reader` rules.

## Diagrams: `diagram.svg`

`diagram.svg` is drawn by hand. No tool generates it, and no tool compares it with the graph: that
it shows what `graph.oce` says is a reviewer's check (D-3).

Boundary inputs are blue pills on the left and fault outputs red pills on the right. Each block is
a rounded rectangle labelled with its instance name over `Class · key parameters`, and signals flow
left to right. The file stands alone, because a forge renders a linked SVG and strips an inline
one.

| # | Rule | Check |
|---|---|---|
| D-1 | `diagram.svg` exists, and the body of the card links it. | `tools/verify` lint |
| D-2 | Every label fits inside the rectangle that encloses it. | `tools/lint/svg_fitment.py` |
| D-3 | The diagram shows the blocks and connections of `graph.oce`. | reader |

## Routine bundles

A routine bundle is a reference part of a sequence. It is a real graph that the engine replays; it
is not a controller, and nothing in it grants permission to deploy. Beside the files of
[Files](#files), it holds:

| File | Origin | What checks it | What a reader may rely on |
|---|---|---|---|
| `source.md` | Authored. | reader | Which clauses, pages and figures the author read, and which choices are the library's own and not the guideline's. |
| `overview.svg` | Authored. | `tools/lint/svg_fitment.py` | A one-screen picture of what the routine does. |
| `class-manifest.json` | Authored. | `tools/lint/routine_schemas.py`; `tools/routines/reference.py` | The class the routine belongs to, and where its class files are. |
| `interface.json` | Authored. | `tools/lint/routine_schemas.py`; `tools/routines/reference.py` | The typed interface of the class. |
| `specialization.json`, `specialization.schema.json` | Authored. | `tools/lint/routine_schemas.py`; `tools/routines/reference.py` | The one specialization of the class that `graph.oce` realizes. |

The formats of the four class files are under [Routine class contracts](#routine-class-contracts).

| # | Rule | Check |
|---|---|---|
| RB-1 | A Real boundary input or output of a routine graph declares its unit with `@unit`. An input declares its accepted range with `@min` and `@max` where the routine has one, and the engine then refuses a frame that carries a value outside it. | not yet checked (L2): the bundle lint requires a unit on every Real boundary; the range is `reader` |
| RB-2 | The connectors of `interface.json` are the boundary of the graph: the same names, the same directions, the same primitive types. | not yet checked (L2): the bundle lint |
| RB-3 | A Real boundary uses the unit `1`, `m3/s` or `K`. A fraction is 0 to 1, never 0 to 100. | not yet checked (L2): the bundle lint |
| RB-4 | A comparison block of a routine graph writes its hysteresis `h` and writes it as zero. A routine adds no hysteresis the guideline does not state. | `tools/routines/reference.py` |
| RB-5 | `status` is `reference`. No routine bundle is promoted, and no graph is renamed to suggest deployment. | `tools/routines/reference.py` checks the status; the rest is `reader` |
| RB-6 | No file named `routine.cxf.jsonld` exists under `routines/g36/`. | `tools/lint/routines.py` |
| RB-7 | The class manifest's identity and revision agree with `interface.json` and `specialization.json`, and its artifact paths resolve to the bundle's own directory. | `tools/lint/routine_schemas.py`; `tools/routines/reference.py` |

## Point dictionaries: `points/<family>.points.json`

```json
{
  "schema": "ocl/points/v1",
  "equipment": "vav",
  "namespaces": {
    "brick": {
      "iri": "https://brickschema.org/schema/Brick#",
      "verified_version": "1.4.4"
    },
    "s223": {
      "iri": "http://data.ashrae.org/standard223#",
      "verified_version": "1.0.0-ppr.2.1"
    },
    "quantitykind": {
      "iri": "http://qudt.org/vocab/quantitykind/",
      "verified_version": "QUDT 3.1.4"
    },
    "unit": {
      "iri": "http://qudt.org/vocab/unit/",
      "verified_version": "QUDT 3.1.4"
    }
  },
  "imports": ["points/zone.points.json"],
  "aliases": [
    {
      "name": "zone_temp",
      "target": "points/zone.points.json#zone_temp"
    }
  ],
  "points": [
    {
      "name": "zone_airflow",
      "description": "VAV box measured supply airflow",
      "kind": "real",
      "unit": "L/s",
      "qudt_unit": "L-PER-SEC",
      "brick": "Supply_Air_Flow_Sensor",
      "s223": {
        "pattern": "Sensor observes QuantifiableObservableProperty at the VAV box discharge via hasProperty",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "VolumeFlowRate",
        "unit": "L-PER-SEC",
        "medium": "Fluid-Air",
        "aspects": []
      },
      "provisional": false,
      "notes": "Attach isPointOf the VAV box; hosts convert CFM before binding."
    }
  ]
}
```

There is one dictionary shape. A dictionary that imports nothing has an empty `imports` list and an
empty `aliases` list.

| # | Rule | Check |
|---|---|---|
| P-1 | A dictionary has exactly the top-level keys `schema`, `equipment`, `namespaces`, `imports`, `aliases` and `points`, and optionally `notes`. `schema` is `ocl/points/v1`. | not yet checked (L3): `tools/lint/point_semantics.py` |
| P-2 | A concrete point's canonical identity is `points/<family>.points.json#<name>`. `name` is snake case, with no suffix for a measured value, `_sp` for a setpoint, `_cmd` for a command, `_status` for a status and `_fbk` for a feedback. | `tools/lint/point_semantics.py` checks the record's shape; the suffix convention is `reader` |
| P-3 | `imports` is a list of unique paths `points/<family>.points.json`, relative to the repository root. An absolute path, a `..` segment, a URL, a file that does not exist, a dictionary's own path and an import cycle are each refused. | `tools/lint/point_semantics.py` |
| P-4 | `aliases` is a list of closed `{name, target}` objects. A `name` is unique and is not the name of a concrete point of the same dictionary. A `target` is a fully qualified point reference whose dictionary is in `imports` and whose fragment names a concrete point. An alias never targets an alias. | `tools/lint/point_semantics.py` |
| P-5 | A bare name resolves in one dictionary only: first as a concrete point of that dictionary, then as one of its aliases. No other dictionary is searched. A reference that is malformed, missing or ambiguous is refused. | `tools/lint/point_semantics.py`; `tools/verify` lint resolves each card's points |
| P-6 | `namespaces` records the ontology IRIs and the versions the terms were checked against, and equals the values of `routines/ontology/ontology-pins.json`, the one authority for them. | `tools/lint/point_semantics.py` |
| P-7 | `brick` is a Brick class local name. `s223` is an object `{pattern, property_class, quantitykind, unit, medium, aspects}`, with `enumerationkind` for an enumerated property. A command or a setpoint maps to an actuatable property class, and an actuatable quantity has a quantity kind and a unit. | `tools/lint/point_semantics.py`, for the closed shapes, the direction rule and the reviewed VAV, SYS and zone mappings |
| P-8 | Every term is reviewed against the evidence the pin record names, never written from memory. `provisional: true` marks an entry with a real ambiguity, and the entry's `notes` say what it is. | reader |

A fault card keeps bare names. The book links each to the concrete point it resolves to and lists
aliases without copying the target's record.

**Role points** are an exception in `points/sys.points.json` only. The cross-equipment
sensor-health rules bind role names (`sensor_value`, `sensor_value_a`, `sensor_value_b`,
`equip_active`) where another rule binds a canonical point, because one graph is deployed against
many real points. A role entry has `brick: null` and `s223: null`. The host's instance
configuration records each binding, and that record is what resolves the rule's `adjudicates`
points.

**`derived: true`** marks a point the host computes: an aggregate across zones, or a physical
transform such as a saturation temperature from a pressure. The entry's `notes` state the
derivation and its site-specific inputs. A rule consumes a derived point exactly as it consumes a
physical one, and the derivation never appears in a graph.

`tools/lint/point_semantics.py` reads local files with no network access. It does not prove that an
external term exists in its ontology, evaluate topology or SHACL, or certify a building instance.
The engine executes CXF and does not read a dictionary.

## Registries

`faults/registry.json` (`ocl/registry/v1`) is an object with `schema` and `rules`. Each row of
`rules` has exactly `id`, `family`, `name`, `method` and `status`. When a card is added or changes
its name, method or status, its row changes in the same pull request.

A family chapter, `faults/<family>/README.md`, is authored prose with an index table of the
family's bundles: id, name, severity, method and status. That the table agrees with the cards is
`not yet checked (L3)`: `tools/lint/registry.py`.

`routines/registry.json` (`ocl/routine-registry/v1`) is an object with exactly `schema` and
`routines`. Each row has exactly `id`, `name`, `status` and `directory`. `status` is `reference`.
A directory is a unique, safe path below `g36/`, relative to `routines/`, and holds one routine
bundle. A scope anchor of `scope.json` is never a row.

`routines/generated-registry.json` (`ocl/generated-routine-registry/v1`) is an object with exactly
`schema` and `deployments`. `deployments` is an empty list. It is the only inventory that may ever
drive routine execution, and an entry in any other file is not a deployment and grants no
permission to deploy. An empty inventory means that no deployment scenario was tested.

| # | Rule | Check |
|---|---|---|
| R-1 | The rows of `faults/registry.json` and the fault bundles are in one-to-one correspondence. | `tools/lint/registry.py` |
| R-2 | A row of `faults/registry.json` has exactly the five keys above. | not yet checked (L3): `tools/lint/registry.py` |
| R-3 | `routines/registry.json` has the shape above, and each directory holds a complete routine bundle. | `tools/routines/reference.py` |
| R-4 | `routines/generated-registry.json` has the shape above and `deployments` is empty. | `tools/lint/routines.py` |

## Routine catalog

The routine catalog is the set of files under `routines/` that are not bundles: the source pins and
the source inventory, the scope and coverage manifests, the class contract schemas, and the
ontology pins. None of them changes a fault bundle.

Three pins have three purposes. `ENGINE_PIN`, at the repository root, names the engine.
`routines/g36/SOURCE_RELEASE_PIN` names the stable Modelica Buildings release baseline.
`routines/g36/SOURCE_DEVELOPMENT_PIN` names the reviewed development baseline for material the
release lacks. Each source pin file holds one lower-case 40-digit hexadecimal Git commit and a
line feed, and the two commits differ. No other pin file exists under `routines/g36/`.
**Check:** `tools/lint/routines.py`, which also refuses the file names `DONOR_PIN` and `SOURCE_PIN`.

### Routine class contracts

Six JSON Schema resources, each declaring `https://json-schema.org/draft/2020-12/schema`:

| Path | `$id` |
|---|---|
| `routines/schemas/common.schema.json` | `https://open-control-library.example/schemas/routine-common-v1.json` |
| `routines/schemas/class-manifest.schema.json` | `https://open-control-library.example/schemas/routine-class-manifest-v1.json` |
| `routines/schemas/interface.schema.json` | `https://open-control-library.example/schemas/routine-interface-v1.json` |
| `routines/schemas/specialization.schema.json` | `https://open-control-library.example/schemas/routine-specialization-v1.json` |
| `routines/schemas/routine-semantic-profile.schema.json` | `https://open-control-library.example/schemas/routine-semantic-profile-v1.json` |
| `routines/schemas/routine-derivation-manifest.schema.json` | `https://open-control-library.example/schemas/routine-derivation-manifest-v1.json` |

A reference uses a fragment of the same resource or one of the six absolute ids, and resolves from
an in-memory registry. Validation fetches nothing from a network or a file system. Objects are
closed unless a contract says otherwise.

A canonical class id has the form `G36-05-(01..22)-<UPPERCASE-HYPHENATED-CLASS-SLUG>`. A scope id
is not a canonical id. A canonical id names a parameterized engineering class and never encodes a
fixed parameter value. The integer `revision` of a class is 1: until release 1.0.0 a class
contract changes in place, as every format does. Parameter, connector, type and dimension ids are
bounded lower-case snake case, and are unique within their lists. A stable repeated-member id is
bounded lower-case hyphenated text that begins with a letter; a dense numeric index is not a member
id. Enum member ids and symbols are unique within their enum.

**Check:** `tools/lint/routine_schemas.py` checks all six resources with
`Draft202012Validator.check_schema`, refuses duplicate JSON keys and non-finite numbers before
schema validation, and adds the cross-document checks named in each subsection below. It accepts
any positive `revision` today; that the value is 1 is `not yet checked (L3)`. That a canonical id
is never reused is a `reader` rule: the checker holds no history.

#### Class manifests (`ocl/routine-class-manifest/v1`)

A class manifest has exactly `schema`, `id`, `revision`, `section`, `source` and `artifacts`.
`section` is `5.1` through `5.22`, and its number agrees with the section in `id`.

`source` is a closed union selected by `kind`:

- `upstream` records `snapshot` (`release` or `development`), a lower-case 40-digit Git revision,
  a Modelica class path, and one or more file locators. Each locator holds a safe path below
  `Buildings/Controls/OBC/ASHRAE/G36/` and a Git blob id `sha1:<40 lower-case hex digits>`.
- `independent` records one or more safe paths relative to the repository root.

A duplicate source path, an absolute path, a backslash, a control character, and an empty, `.` or
`..` segment are each refused. `artifacts` has exactly `interface`, `specialization_schema` and
`specialization_config`. Their paths share one class directory and end in `interface.json`,
`specialization.schema.json` and `specialization.json`. **Check:** `tools/lint/routine_schemas.py`.

#### Interfaces (`ocl/routine-interface/v1`)

An interface has exactly `schema`, `canonical_id`, `revision`, `types`, `dimensions`, `parameters`
and `connectors`. Types and enums are local to the interface; there is no global type catalog.

The primitive symbols are exactly `real`, `integer` and `boolean`. A named alias selects one
primitive and may record non-empty trimmed `quantity`, `unit` and `display_unit` strings, which
assert no QUDT, Brick or ASHRAE 223 meaning. An enum declares a non-empty ordered list of unique
stable member ids and unique symbols. An enum value is a member id; the interface assigns no
integer code.

A type use is a primitive or a reference to a local named type. A shape is scalar, or an array
with an ordered list of one or two dimension ids. A fixed dimension declares a positive integer
extent and a non-empty ordered `members` list of that length. A parameter-driven dimension names a
scalar Integer parameter and declares no members; the specialization owns its member list. A
member is an authored identity, not an array ordinal, and the authored order is the contract
order. Within one interface and its specialization, member ids are unique across every dimension.
A rank above two, a zero extent, a ragged matrix and an arithmetic dimension expression are each
refused.

A parameter has a unique id, a type use, a shape, `fixed` or `configurable` configurability, an
optional typed default, and optional numeric minimum and maximum. A fixed parameter has a default
and cannot be assigned by a specialization. A configurable parameter without a default is assigned
by the specialization.

A connector has a unique id, an `input` or `output` direction, a type use, a shape and an explicit
presence. Presence is `always`, or `when` with a closed guard expression. A guard supports `and`,
`or`, `not`, and the comparisons `eq`, `ne`, `lt`, `lte`, `gt` and `gte`. An operand is a scalar
parameter reference or a typed scalar literal; an ordering comparison takes numeric operands, and
Integer and Real operands are compatible. A runtime signal, a connector, time, a point id, an
operating state and host or fault logic cannot appear in a guard.

**Check:** `tools/lint/routine_schemas.py` validates guard structure, references and operand
compatibility. It does not evaluate a guard, and no routine bundle has a connector whose presence
is not `always`.

#### Specialization inputs (`ocl/routine-specialization/v1`)

A specialization has exactly `schema`, `canonical_id`, `revision`, `parameters` and `members`.
`parameters` is an ordered list of unique parameter ids and JSON values. `members` binds each
parameter-driven dimension id to a non-empty ordered list of stable member ids, unique across the
specialization and against the members of every fixed dimension of the interface. A fixed
dimension is not a valid target.

The canonical id and revision of the interface and the specialization agree with the class
manifest. A specialization is input only: it holds no connector binding, point id, resolved
connector set, source map, generated CXF, runtime state, engine identity or deployment identity.

**Check:** `tools/lint/routine_schemas.py` checks that each parameter exists, that a fixed
parameter is not overridden, that every required assignment is present, that values fit their
primitive or enum and their numeric bounds, that dimensions resolve to positive extents with the
right member counts, that arrays are rectangular, and that every number is finite.

#### Ontology identities and local vocabulary

`routines/ontology/ontology-pins.json` (`ocl/ontology-pins/v1`) is the one record of the ontology
identities this library was checked against. It is the authority for the releases, commits, blob
ids and hashes, and this file does not repeat them.

| Authority | Namespace | Version |
|---|---|---|
| Brick | `https://brickschema.org/schema/Brick#` | 1.4.4 |
| ASHRAE 223 compatibility | `http://data.ashrae.org/standard223#`, with the Guideline 36 extension `http://data.ashrae.org/standard223/1.0/extensions/g36#` | `1.0.0-ppr.2.1` |
| QUDT | `http://qudt.org/vocab/quantitykind/` and `http://qudt.org/vocab/unit/` | 3.1.4 |
| OCL (this library) | `urn:open-control-library:ontology:` | `0.1.0-draft`, the file `routines/ontology/ocl-vocabulary.ttl` |

The ASHRAE 223 artifact is a public-review compatibility baseline, not the published standard. It
imports a later QUDT than the one pinned here; the pin record keeps that as an observation and it
does not replace the library's QUDT authority.

`ocl-vocabulary.ttl` holds only the library's own profile, connector-binding, software-signal,
derived-signal, aggregate, derivation and policy terms. It imports nothing. Its SHA-256 is in the
pin record, and a change to its bytes updates that hash in the same pull request.

`routines/ontology/shacl/open-control-routine-shapes.ttl` is the SHACL Core graph for the two
synthetic semantic fixtures. Its shape identifiers use `urn:open-control-library:shacl:routine:`,
and every OCL term it uses is declared by `ocl-vocabulary.ttl`. It defines no SPARQL constraint,
no SHACL-AF rule and no SHACL-JS.

**Check:** `tools/lint/routine_semantics.py` checks the closed pin record, recomputes the
vocabulary's hash, parses the local Turtle, and validates with no import and no inference.

#### Routine semantic profiles (`ocl/routine-semantic-profile/v1`)

A semantic profile has a stable JSON-LD `@id`, the type `ocl:RoutineSemanticProfile`, a canonical
class id and revision, the exact reference `routines/ontology/ontology-pins.json`, and one or more
connector roles. Its context is one closed embedded object; a string, list, nested, remote or
imported context is refused.

A connector role id uses the connector id syntax and is unique. Each role has a bounded non-empty
`semantic_role`, a `mapping_status` of `verified` or `provisional`, and a closed list of bounded
topology requirements. Here `verified` means that the author reviewed the mapping against the
pinned evidence and `provisional` that it still needs review; neither certifies a building, and
neither is the verified bar of a bundle. A physical role has at least one location, topology or
ownership requirement; a software or derived role may have none.

Direction is connector dataflow, `input` or `output`, and does not decide the ASHRAE 223 property
class: an active setpoint may be an input while its property is actuatable. Requirement is `R`,
`A`, `O`, `N`, `S`, `D` or `P`. Cardinality records integer `minimum` and `maximum`, the first not
above the second. A binding is a closed union:

- `physical-or-bms-point` carries one reference `points/<family>.points.json#<point_key>` and
  closed Brick and ASHRAE 223 mappings;
- `software-signal` carries an OCL class and no physical mapping;
- `derived-signal` carries an OCL class, an output id and a reference to a local derivation
  manifest.

A physical mapping allows only `s223:QuantifiableObservableProperty`,
`s223:QuantifiableActuatableProperty`, `s223:EnumeratedObservableProperty` and
`s223:EnumeratedActuatableProperty`. A quantifiable mapping has a QUDT quantity kind and unit. An
enumerated mapping has an enumeration kind. Both record the medium as a CURIE or an explicit
`null`. The allowed aspects are `s223:Aspect-Setpoint`, `s223:Aspect-Delta` and
`s223:Aspect-Maximum`. A topology string is an authoring requirement, not a topology instance.

#### Derivation manifests (`ocl/routine-derivation-manifest/v1`)

A derivation manifest identifies one `ocl:DerivedSignal` or `ocl:DerivedAggregate` output. It
records the canonical class revision, the exact reference `routines/ontology/ontology-pins.json`,
a function id and version, ordered typed inputs with stable source ids, stable members,
exclusions, data-quality handling, freshness and alignment limits in seconds, the readiness and
in-domain policy, the output unit and conversion policy, the output scope and the reset behaviour.

Member-linked inputs, exclusions, member output scopes and source-triggered resets resolve inside
the manifest. Ids are unique. The data-quality and readiness minimums do not exceed the member
population and agree with each other. A profile's derived output id and manifest fragment equal the
manifest's output id, and a manifest output is referenced by exactly one derived connector role.

**Check, for profiles and manifests:** `tools/lint/routine_semantics.py` applies the six-resource
schema registry, refuses unsafe JSON-LD before parsing, and passes each fixture's graph to
pySHACL 0.31.0 with imports, inference, advanced features, JavaScript and in-place mutation off.
The two fixtures under `tools/lint/tests/fixtures/routine_semantics/` are synthetic; their point
references are examples of syntax and are not resolved against the dictionaries. No external
ontology is vendored or fetched, so a Brick, ASHRAE 223 or QUDT CURIE is checked for syntax and for
the class and aspect policy above, not for existence in its ontology.

#### Fixtures

`tools/lint/tests/fixtures/routine_schemas/` holds one coherent synthetic fixture set that the
schema linter validates. Those documents are test evidence for the contracts. They do not appear
under `routines/g36/`, in a registry, a coverage claim, the source inventory, the book or any
production catalog. A class file under `routines/g36/` belongs to a registered routine bundle.
**Check:** `tools/lint/routine_schemas.py` refuses an unregistered class file.

### `routines/g36/source-inventory.json` (`ocl/g36-source-inventory/v1`)

The source inventory records two independent Git-tree snapshots of
`https://github.com/lbl-srg/modelica-buildings.git`. Its top-level object has exactly these keys,
in this order: `schema`, `repository`, `source_root`, `inventory_scope`, `dependency_closure`,
`license` and `snapshots`.

- `repository` is `https://github.com/lbl-srg/modelica-buildings.git`;
- `source_root` is `Buildings/Controls/OBC/ASHRAE/G36`;
- `inventory_scope` is `source-root-regular-files`;
- `dependency_closure` is `not-inventoried`.

`license` has exactly `upstream_path`, `retained_path`, `git_blob_sha1` and `sha256`, in that
order. `upstream_path` is `Buildings/legal.html` and `retained_path` is
`routines/g36/LICENSE-BUILDINGS.html`. The retained file equals the Git blob bytes at both pins.
`git_blob_sha1` is `sha1:<40 lower-case hex digits>` and `sha256` is
`sha256:<64 lower-case hex digits>`.

`snapshots` has exactly two rows, `release` then `development`. Each row has exactly these keys in
order: `role`, `revision`, `root_tree_sha1`, `file_count`, `total_bytes`, `modelica_file_count`,
`package_order_count` and `files`. `revision` equals the matching pin file. `root_tree_sha1` is the
Git tree id of `source_root`.

Each `files` row has exactly `path`, `mode`, `bytes`, `git_blob_sha1` and `sha256`, in that order.
A path is the full upstream POSIX path below `source_root`; paths are sorted and unique. An empty,
`.` or `..` segment, an absolute path, a backslash and a control character are each refused. Only
regular `100644` Git blobs are supported. `bytes` and both hashes are computed from Git object
bytes, not from working-tree files. `file_count` is the number of rows, `total_bytes` the sum of
their `bytes`, `modelica_file_count` the number of paths ending in `.mo`, and `package_order_count`
the number ending in `/package.order`.

The two snapshots stay separate: neither is a fallback for the other. The inventory covers nothing
outside `source_root` and makes no claim about Modelica declarations, classes, package members,
references, inheritance or executable coverage. The file uses two-space indentation, the key order
above and one final line feed, and holds no timestamp, branch name, checkout path or moving
identity.

**Check:** `tools/lint/g36_source.py --check` verifies each checkout's origin and exact `HEAD`,
derives every record through Git object commands, and compares the inventory and the retained
notice byte for byte. `--write` regenerates both from two supplied checkouts. A change to a source
pin regenerates both snapshots.

### `routines/g36/scope.json` (`ocl/g36-scope/v1`)

The scope manifest makes the Section 5 planning boundary discoverable without claiming a class.
Its top-level object has exactly `schema`, `profile`, `status` and `sections`. `profile` is
`ASHRAE Guideline 36-2021 Section 5`, `status` is `planned`, and `sections` has exactly 22 rows,
`5.1` through `5.22` in order.

Each row has exactly `id`, `section`, `name`, `status`, `source_disposition` and `destination`.
Ids, sections and destinations are each unique. `name` is a non-empty display name written here,
and `status` is `planned`. A scope id is a planning anchor and is never a canonical class id.

| Section | Scope id | Destination | Source disposition |
|---|---|---|---|
| 5.1 | `G36-SCOPE-05-01` | `g36/shared/general` | `mixed` |
| 5.2 | `G36-SCOPE-05-02` | `g36/zones/ventilation` | `upstream-partial` |
| 5.3 | `G36-SCOPE-05-03` | `g36/zones/thermal` | `upstream-broad` |
| 5.4 | `G36-SCOPE-05-04` | `g36/zone-groups` | `upstream-broad` |
| 5.5 | `G36-SCOPE-05-05` | `g36/terminal-units/cooling-only` | `upstream-broad` |
| 5.6 | `G36-SCOPE-05-06` | `g36/terminal-units/reheat` | `upstream-broad` |
| 5.7 | `G36-SCOPE-05-07` | `g36/terminal-units/parallel-fan-cv` | `upstream-broad` |
| 5.8 | `G36-SCOPE-05-08` | `g36/terminal-units/parallel-fan-vv` | `upstream-broad` |
| 5.9 | `G36-SCOPE-05-09` | `g36/terminal-units/series-fan-cv` | `upstream-broad` |
| 5.10 | `G36-SCOPE-05-10` | `g36/terminal-units/series-fan-vv` | `upstream-broad` |
| 5.11 | `G36-SCOPE-05-11` | `g36/terminal-units/dual-duct-snap` | `upstream-broad` |
| 5.12 | `G36-SCOPE-05-12` | `g36/terminal-units/dual-duct-mix-inlet` | `upstream-broad` |
| 5.13 | `G36-SCOPE-05-13` | `g36/terminal-units/dual-duct-mix-discharge` | `upstream-broad` |
| 5.14 | `G36-SCOPE-05-14` | `g36/terminal-units/dual-duct-cold-min` | `upstream-broad` |
| 5.15 | `G36-SCOPE-05-15` | `g36/ahus/system-modes` | `upstream-embedded` |
| 5.16 | `G36-SCOPE-05-16` | `g36/ahus/multizone-vav` | `upstream-broad` |
| 5.17 | `G36-SCOPE-05-17` | `g36/ahus/dual-fan-dual-duct` | `independent-authoring` |
| 5.18 | `G36-SCOPE-05-18` | `g36/ahus/single-zone-vav` | `upstream-broad` |
| 5.19 | `G36-SCOPE-05-19` | `g36/exhaust-fans/constant-speed` | `independent-authoring` |
| 5.20 | `G36-SCOPE-05-20` | `g36/plants/chilled-water` | `development-source` |
| 5.21 | `G36-SCOPE-05-21` | `g36/plants/hot-water` | `independent-authoring` |
| 5.22 | `G36-SCOPE-05-22` | `g36/fan-coil-units` | `upstream-partial` |

A disposition classifies the source plan; it identifies no source class and proves no
implementation. A destination is a safe POSIX path below `g36/`, relative to `routines/`, and
reserves a place without requiring a directory. An absolute path, a backslash, and an empty, `.`
or `..` segment are each refused. **Check:** `tools/lint/routines.py`.

### `routines/g36/coverage.json` (`ocl/g36-coverage/v1`)

Coverage has exactly `schema`, `profile`, `status`, `scope` and `claims`. `profile` and `status`
equal those of `scope.json`, `scope` is `scope.json`, and `claims` is an empty list. Coverage
repeats no scope row and makes no claim of completeness, implementation or evidence. The two
routine bundles read the 2018 edition and add no claim about the 2021 profile. The path
`routines/g36/generic/air-economizer-high-limits` does not exist. **Check:**
`tools/lint/routines.py`.

### What the routine catalog does not hold

Deployable class bundles, production semantic profiles and derivation manifests, instances that
map a source to a class, a source specializer, connectors resolved against a building, generated
deployment rows, building-instance conformance and live execution are not defined by this
contract. The reference routines, their typed interfaces, their empty specializations, their
graphs and their vectors do not complete any of them.

## Clusters: `clusters/clusters.json` (`ocl/clusters/v1`)

An object with `schema`, `source` and `clusters`. Each cluster is
`{id, name, trigger, members, playbook, prevalence, energy_impact}`. `trigger` is the fault that
fires first; fixing it is expected to clear `members` within 24 to 48 hours. A card's `clusters`
entries are ids of this file (C-5). That a trigger, a member or a playbook of a cluster names
something that exists is `not yet checked (L4)`: the bundle lint.

## Playbooks: `playbooks/<slug>.md`

Prose with a header block (Applies to, Fix complexity, Typical time, Typical cost, Energy impact)
and a four-step workflow: verify, remote fix, on-site service, confirm resolution. A fault card
names playbooks by slug (C-5), and a playbook lists the fault ids it applies to. That the two
lists agree is `not yet checked (L4)`: the bundle lint.

## Verification

Verification is the gate running [the bar](#the-bar) on every bundle with the pinned `oce` tool.
`.agents/gate.sh` holds the commands in the form CI runs them, and `AGENTS.md` says what CI runs
when. The gate builds `oce` from the engine checkout that
`bash .github/scripts/pinned-sources.sh fetch` places beside this repository at the commit in
`ENGINE_PIN`, with the Rust toolchain that checkout names, so the pin selects the compiler too.

Until L2 adds those steps, the gate replays the corpus with the inherited `tools/verify`, which
this contract does not describe. A stamp is a claim under this contract only once the checks of
[the bar](#the-bar) run on its bundle.

There is no batch mode: `oce verify` takes one vector file and one graph, and the gate loops over
the bundles the registries list. A bundle that the loop does not reach is not verified, whatever
its card says.

| # | Rule | Check |
|---|---|---|
| X-1 | The gate runs the bar on every bundle a registry lists. | not yet checked (L2): a lint fails unless every registry row is covered; L3 removes every other path |
| X-2 | A scenario that fails, a mutant that survives and a file the engine refuses each fail the gate. | not yet checked (L2): V-3 |
| X-3 | No tool in this repository loads, replays or compares a graph. | not yet checked (L4): no `Cargo.toml` and no `.rs` file is tracked |

## Open dependencies

This contract is written ahead of engine work it depends on. Each row names the engine item, what
the contract assumes until it lands, and what changes when it does. Issue numbers are in the
engine repository unless a row says otherwise.

| Engine item | State when this was written | What this contract assumes until it lands | What changes when it lands |
|---|---|---|---|
| `ENGINE_PIN` | It names engine release `v0.4.0`, which has no `oce` binary and holds neither `docs/design/vectors.md` nor `docs/design/content-id.md`. | Every check that names `oce` is `not yet checked`, and the cited documents are read from the engine's `development` branch. | L2 advances the pin to a commit that has `oce verify` with mutant mode, `oce id` and the namespace change. From then on the cited documents are read at the pin. |
| The vector format, `oce/vectors/v1` (#519) | Specified in `docs/design/vectors.md` and ruled on. Not built. | The format is exactly what the note specifies. | Nothing here, unless the implementation amends the note. The note marks its canonical spelling of Real numbers as unconfirmed; that spelling affects the bytes `spec_id` hashes and no rule here. |
| Bind, replay, compare and the receipt (#520) | Not built. Waits on #519. | The receipt has the header and body fields that `docs/design/vectors.md` tabulates, and the stamp copies `content_id`, `engine_version` and `spec_id` from its header. | If a field is renamed, the stamp field of the same name and rules V-5 and V-6 follow it. |
| Mutant mode (#521) | Not built. Waits on #520. | `vectors.json` lists mutants under `mutants`, each a path inside the bundle; the receipt reports `total`, `caught` and `survived`. Until this lands the format refuses the `mutants` key, so no set that lists mutants can be replayed with them ignored. | Nothing, if built as specified. |
| The `oce verify` command (#522) | Not built. Waits on #520 and #521. | `oce verify <vectors> --subject <graph> [--out <directory>] [--require-mutants-caught]`, the receipt on standard output or in `<directory>/receipt.json`, exit 0, 1 or 2. | If a flag is renamed, the commands in [Files](#files) and [The bar](#the-bar) follow it. |
| `oce id` (#513) | Not built. | `oce id <file>` prints the content id and a line feed. | Nothing, if built as described. |
| `oce import` (#514) | Not built. | Nothing in this contract. L3 uses it once to write each fault's text form from its present CXF; no bundle depends on it afterwards. | Nothing here. |
| The CXF namespace `http://data.ashrae.org/S231#` and the translation-software label (#527) | In progress. The emitter at the engine's `development` branch still binds `http://data.ashrae.org/S231P#`. | An emitted document binds `S231` to `http://data.ashrae.org/S231#` and names, on its root, the tool and version that wrote it; an importer refuses a document that binds `S231P#`. No file here is written by hand in either namespace: G-3 compares bytes with what the pinned tool emits. | L2 pins an engine commit that has it. No content id depends on the vocabulary namespace or on the label; #527 has to show that. |
| `derived_from` (#529) | Queued. The text form's statement is spelled `extends` today. | The statement and the card field are both `derived_from`. The word `extends` is not used for a bundle's base anywhere in this repository. | L2 pins an engine commit that has it. Nothing here changes. |
| The spelling of vector and matrix port names (#528) | A design note that is not yet written. The engine spells a vector connector `u1` today; the OBC specification's §8.6 spells it `u_1`. | This contract names no block port (G-5): a text form uses whatever `oce catalog` prints at the pin. | If the spelling changes, every text form that connects such a port is rewritten in the pull request that advances the pin, and the emitted bytes move with it. Whether a content id moves is for the note to say. L3 does not start until the note is ruled on. |
| Conditional components (#487) | Design note first; not built in the authoring model. | No bundle has one (G-12). | When the engine admits them, the OBC rule applies: the connections of a removed instance are dropped, and an active input left with no source is refused. This contract then says which bundles may use one. |
| Enum boundary ports (#465) | Open. Until it lands, binding a vector set refuses an Enum input or output with `enum-port-unsupported`. | A routine input that carries a mode is an Integer, and the card's `integer_codes` give its codes. | Nothing here until a bundle wants an Enum port; that is then a change to this contract. |
| `within`, `when` and file-backed series (engine tracking issue #456, items 6 and 7) | Not built. The format refuses the keys. | Every expectation is exact (T-6). No bundle holds reference series (B-4). | T-6 is rewritten to admit a band with named tolerances, and B-4 to define `reference/`. |
| The registry index and `oce fetch` (#515) | A design note that is not yet written. | `derived_from` is opaque (C-9). The three registry files are as [Registries](#registries) states. | L5 (#110 in this repository) restates the registries from that note. |
| The replay of this corpus in the engine's release gate (#516), and the re-vendoring of two bundles into the engine (#517) | Both wait on L2 and L3. | Nothing: they consume this library's output. | Nothing here. |
