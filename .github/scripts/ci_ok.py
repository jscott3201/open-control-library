#!/usr/bin/env python3
"""The `CI OK` result check for pr-gate.yml and release-gate.yml.

Reads ``NEEDS_JSON``, which the `CI OK` job sets to ``${{ toJSON(needs) }}``: one entry per job it
needs, each with a ``result``. The check passes only when at least one job is listed and every
result is exactly ``success``.

Neither gate workflow filters a job, so a skip is never legitimate: ``skipped`` (a job that did not
start, for example because a job it needs failed), ``failure``, ``cancelled``, a missing result and
malformed input all fail. check_workflow_gates.py asserts that the job's ``needs`` lists every
other job in its workflow. Standard library only.
"""

from __future__ import annotations

import json
import os
import sys


class CheckError(Exception):
    """The aggregate must not report success."""


def check(needs: object) -> list[str]:
    """Return one report line per job; raise CheckError unless every job succeeded."""
    if not isinstance(needs, dict):
        raise CheckError(f"NEEDS_JSON is not an object of job results: {type(needs).__name__}")
    if not needs:
        raise CheckError("no gating job results reported")
    report, bad = [], {}
    for job, data in sorted(needs.items()):
        result = data.get("result") if isinstance(data, dict) else None
        report.append(f"{job}: {result}")
        if result != "success":
            bad[job] = result
    if bad:
        raise CheckError(f"gating jobs not green: {bad}\n" + "\n".join(report))
    return report


def main() -> int:
    try:
        report = check(json.loads(os.environ["NEEDS_JSON"]))
    except (CheckError, KeyError, ValueError) as error:
        print(f"CI OK: FAIL: {error}", file=sys.stderr)
        return 1
    print("\n".join(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
