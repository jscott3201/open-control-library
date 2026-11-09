<!-- Title: type(scope): what it does — imperative. One pull request per issue; fix rounds push to this branch. -->

Closes #

## What and why

<!-- Two paragraphs at most. Link the design note if one exists. -->

## Gate

```text
<!-- Paste the TAIL of `bash .agents/gate.sh full`, run first-hand on the head being merged:
     the STEPS list, the OBSERVED TOTALS and the closing GATE line. Not a summary. -->
```

## Changelog

<!-- The `CHANGELOG.md` entry added under `[Unreleased]`, by rule id where a rule changed, cited
     `(GitHub #N)` — or "none: no rule, vector, format, tool or published claim changes". -->

## Checklist

- [ ] No expectation, tolerance or vector was edited to make a scenario pass; a failing scenario is reported above
- [ ] Rule logic is unchanged — or the issue says to change it and the changelog lists the rule ids
- [ ] Routine `status` stays `reference`; `routines/generated-registry.json` stays empty; `ENGINE_PIN` is unchanged — or the issue says otherwise
- [ ] No new format revision; no code that loads, replays, compiles or authors control graphs
- [ ] No absolute paths or UUID-shaped strings; documents that named what changed are updated here
- [ ] `CI OK` reported green on this head (not merely nothing red), and `book-pages` too if it ran
