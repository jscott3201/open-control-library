#!/usr/bin/env python3
"""Generate or check the pinned G36 Git-tree inventory."""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlsplit


REPO_ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "https://github.com/lbl-srg/modelica-buildings.git"
REPOSITORY_SLUG = "lbl-srg/modelica-buildings"
SOURCE_ROOT = "Buildings/Controls/OBC/ASHRAE/G36"
LEGAL_UPSTREAM_PATH = "Buildings/legal.html"
INVENTORY_PATH = Path("routines/g36/source-inventory.json")
RETAINED_LEGAL_PATH = Path("routines/g36/LICENSE-BUILDINGS.html")
RELEASE_PIN_PATH = Path("routines/g36/SOURCE_RELEASE_PIN")
DEVELOPMENT_PIN_PATH = Path("routines/g36/SOURCE_DEVELOPMENT_PIN")

SCHEMA = "cxf-library/g36-source-inventory/v1"
INVENTORY_SCOPE = "source-root-regular-files"
DEPENDENCY_CLOSURE = "not-inventoried"
TOP_LEVEL_KEYS = (
    "schema",
    "repository",
    "source_root",
    "inventory_scope",
    "dependency_closure",
    "license",
    "snapshots",
)
LICENSE_KEYS = ("upstream_path", "retained_path", "git_blob_sha1", "sha256")
SNAPSHOT_KEYS = (
    "role",
    "revision",
    "root_tree_sha1",
    "file_count",
    "total_bytes",
    "modelica_file_count",
    "package_order_count",
    "files",
)
FILE_KEYS = ("path", "mode", "bytes", "git_blob_sha1", "sha256")

PIN_RE = re.compile(r"[0-9a-f]{40}\n")
SHA1_RE = re.compile(r"sha1:[0-9a-f]{40}")
SHA256_RE = re.compile(r"sha256:[0-9a-f]{64}")
TREE_ROW_RE = re.compile(
    rb"(?P<mode>[0-9]{6}) (?P<type>[a-z]+) (?P<oid>[0-9a-f]{40})"
    rb" +(?P<size>[0-9-]+)\t(?P<path>.+)",
    re.DOTALL,
)


class InventoryError(Exception):
    """A deterministic source or artifact validation failure."""


class _DuplicateKeyError(ValueError):
    pass


def _decode(value, label):
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise InventoryError(f"{label}: Git output is not UTF-8") from exc


def _git(checkout, role, *arguments, failure=None):
    try:
        result = subprocess.run(
            ["git", "-C", str(checkout), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        raise InventoryError(f"{role} checkout: unable to execute Git") from exc
    if result.returncode != 0:
        if failure is not None:
            raise InventoryError(failure)
        detail = _decode(result.stderr, f"{role} checkout").strip().splitlines()
        suffix = f": {detail[0]}" if detail else ""
        raise InventoryError(f"{role} checkout: Git command failed{suffix}")
    return result.stdout


def _read_pin(repo_root, relative_path):
    label = relative_path.as_posix()
    try:
        value = (repo_root / relative_path).read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise InventoryError(f"{label}: file is missing") from exc
    except (OSError, UnicodeError) as exc:
        raise InventoryError(f"{label}: unable to read file") from exc
    if PIN_RE.fullmatch(value) is None:
        raise InventoryError(
            f"{label}: must contain one lowercase 40-hex Git commit followed by a newline"
        )
    return value[:-1]


def _repository_slug(url):
    value = url.strip()
    if value.startswith("git@github.com:"):
        path = value.removeprefix("git@github.com:")
    else:
        parsed = urlsplit(value)
        if parsed.hostname != "github.com":
            return None
        path = parsed.path.lstrip("/")
    if path.endswith(".git"):
        path = path[:-4]
    return path.rstrip("/")


def _validate_checkout(checkout, role, revision):
    try:
        requested_root = Path(checkout).resolve(strict=True)
    except (OSError, RuntimeError) as exc:
        raise InventoryError(f"{role} checkout: path does not exist") from exc
    if not requested_root.is_dir():
        raise InventoryError(f"{role} checkout: path is not a directory")

    top_level = _decode(
        _git(
            requested_root,
            role,
            "rev-parse",
            "--show-toplevel",
            failure=f"{role} checkout: path is not a Git checkout",
        ),
        f"{role} checkout",
    ).strip()
    try:
        actual_root = Path(top_level).resolve(strict=True)
    except (OSError, RuntimeError) as exc:
        raise InventoryError(f"{role} checkout: Git top-level path is invalid") from exc
    if actual_root != requested_root:
        raise InventoryError(f"{role} checkout: path must be the Git checkout root")

    origin = _decode(
        _git(
            requested_root,
            role,
            "remote",
            "get-url",
            "origin",
            failure=f"{role} checkout: origin remote is missing",
        ),
        f"{role} checkout",
    ).strip()
    if _repository_slug(origin) != REPOSITORY_SLUG:
        raise InventoryError(
            f"{role} checkout: origin must identify {REPOSITORY_SLUG}"
        )

    actual_head = _decode(
        _git(
            requested_root,
            role,
            "rev-parse",
            "--verify",
            "HEAD^{commit}",
            failure=f"{role} checkout: HEAD is not a commit",
        ),
        f"{role} checkout",
    ).strip()
    if actual_head != revision:
        raise InventoryError(
            f"{role} checkout: HEAD must be {revision}, found {actual_head}"
        )
    return requested_root


def _safe_source_path(path):
    if not isinstance(path, str):
        return "must be a string"
    if path.startswith("/"):
        return "absolute paths are forbidden"
    if "\\" in path:
        return "backslashes are forbidden"
    if any(ord(character) < 32 or ord(character) == 127 for character in path):
        return "control characters are forbidden"
    segments = path.split("/")
    if "" in segments:
        return "empty path segments are forbidden"
    if "." in segments:
        return "dot path segments are forbidden"
    if ".." in segments:
        return "parent traversal is forbidden"
    if not path.startswith(f"{SOURCE_ROOT}/"):
        return f"path must be below {SOURCE_ROOT}/"
    return None


def _git_blob(checkout, role, oid, path, expected_size):
    data = _git(
        checkout,
        role,
        "cat-file",
        "blob",
        oid,
        failure=f"{role} snapshot: unable to read Git blob for {path}",
    )
    if len(data) != expected_size:
        raise InventoryError(
            f"{role} snapshot: Git blob size for {path} is {len(data)}, "
            f"tree records {expected_size}"
        )
    header = f"blob {len(data)}\0".encode("ascii")
    actual_oid = hashlib.sha1(header + data).hexdigest()
    if actual_oid != oid:
        raise InventoryError(
            f"{role} snapshot: Git blob ID for {path} does not match its bytes"
        )
    return data


def _root_tree_oid(checkout, role, revision):
    oid = _decode(
        _git(
            checkout,
            role,
            "rev-parse",
            f"{revision}:{SOURCE_ROOT}",
            failure=(
                f"{role} snapshot: source root {SOURCE_ROOT} is missing at {revision}"
            ),
        ),
        f"{role} snapshot",
    ).strip()
    if re.fullmatch(r"[0-9a-f]{40}", oid) is None:
        raise InventoryError(f"{role} snapshot: source root tree ID is malformed")
    object_type = _decode(
        _git(
            checkout,
            role,
            "cat-file",
            "-t",
            oid,
            failure=f"{role} snapshot: source root object cannot be read",
        ),
        f"{role} snapshot",
    ).strip()
    if object_type != "tree":
        raise InventoryError(f"{role} snapshot: source root must be a Git tree")
    return oid


def _parse_tree_row(row, role):
    match = TREE_ROW_RE.fullmatch(row)
    if match is None:
        raise InventoryError(f"{role} snapshot: malformed Git tree entry")
    path = _decode(match.group("path"), f"{role} snapshot path")
    mode = match.group("mode").decode("ascii")
    object_type = match.group("type").decode("ascii")
    oid = match.group("oid").decode("ascii")
    size_text = match.group("size").decode("ascii")
    if mode != "100644" or object_type != "blob":
        raise InventoryError(
            f"{role} snapshot: unsupported Git entry {path} "
            f"({mode} {object_type}); v1 supports only 100644 blobs"
        )
    if not size_text.isdigit():
        raise InventoryError(f"{role} snapshot: Git blob size is missing for {path}")
    problem = _safe_source_path(path)
    if problem is not None:
        raise InventoryError(f"{role} snapshot: unsafe path {path!r}: {problem}")
    return path, mode, oid, int(size_text)


def _snapshot(checkout, role, revision):
    root_tree_oid = _root_tree_oid(checkout, role, revision)
    tree_output = _git(
        checkout,
        role,
        "ls-tree",
        "-r",
        "-l",
        "-z",
        "--full-tree",
        revision,
        "--",
        SOURCE_ROOT,
        failure=f"{role} snapshot: unable to list source root tree",
    )

    files = []
    seen_paths = set()
    for row in tree_output.split(b"\0"):
        if not row:
            continue
        path, mode, oid, size = _parse_tree_row(row, role)
        if path in seen_paths:
            raise InventoryError(f"{role} snapshot: duplicate Git tree path {path}")
        seen_paths.add(path)
        data = _git_blob(checkout, role, oid, path, size)
        files.append(
            {
                "path": path,
                "mode": mode,
                "bytes": len(data),
                "git_blob_sha1": f"sha1:{oid}",
                "sha256": f"sha256:{hashlib.sha256(data).hexdigest()}",
            }
        )

    files.sort(key=lambda row: row["path"])
    return {
        "role": role,
        "revision": revision,
        "root_tree_sha1": f"sha1:{root_tree_oid}",
        "file_count": len(files),
        "total_bytes": sum(row["bytes"] for row in files),
        "modelica_file_count": sum(row["path"].endswith(".mo") for row in files),
        "package_order_count": sum(
            row["path"].endswith("/package.order") for row in files
        ),
        "files": files,
    }


def _legal_blob(checkout, role, revision):
    output = _git(
        checkout,
        role,
        "ls-tree",
        "-l",
        "-z",
        "--full-tree",
        revision,
        "--",
        LEGAL_UPSTREAM_PATH,
        failure=f"{role} snapshot: unable to inspect {LEGAL_UPSTREAM_PATH}",
    )
    rows = [row for row in output.split(b"\0") if row]
    if not rows:
        raise InventoryError(
            f"{role} snapshot: legal file {LEGAL_UPSTREAM_PATH} is missing at {revision}"
        )
    if len(rows) != 1:
        raise InventoryError(
            f"{role} snapshot: legal path {LEGAL_UPSTREAM_PATH} is ambiguous"
        )
    match = TREE_ROW_RE.fullmatch(rows[0])
    if match is None:
        raise InventoryError(f"{role} snapshot: malformed legal Git tree entry")
    mode = match.group("mode").decode("ascii")
    object_type = match.group("type").decode("ascii")
    oid = match.group("oid").decode("ascii")
    size_text = match.group("size").decode("ascii")
    if mode != "100644" or object_type != "blob" or not size_text.isdigit():
        raise InventoryError(
            f"{role} snapshot: {LEGAL_UPSTREAM_PATH} must be a 100644 Git blob"
        )
    data = _git_blob(
        checkout,
        role,
        oid,
        LEGAL_UPSTREAM_PATH,
        int(size_text),
    )
    return oid, data


def _build_inventory(repo_root, release_root, development_root):
    release_revision = _read_pin(repo_root, RELEASE_PIN_PATH)
    development_revision = _read_pin(repo_root, DEVELOPMENT_PIN_PATH)
    if release_revision == development_revision:
        raise InventoryError("source release and development pins must be distinct")

    release_checkout = _validate_checkout(
        release_root, "release", release_revision
    )
    development_checkout = _validate_checkout(
        development_root, "development", development_revision
    )
    if release_checkout == development_checkout:
        raise InventoryError("release and development checkouts must be separate")

    release = _snapshot(release_checkout, "release", release_revision)
    development = _snapshot(
        development_checkout, "development", development_revision
    )
    release_legal_oid, release_legal = _legal_blob(
        release_checkout, "release", release_revision
    )
    development_legal_oid, development_legal = _legal_blob(
        development_checkout, "development", development_revision
    )
    if release_legal_oid != development_legal_oid or release_legal != development_legal:
        raise InventoryError("release and development legal notices differ")

    inventory = {
        "schema": SCHEMA,
        "repository": REPOSITORY,
        "source_root": SOURCE_ROOT,
        "inventory_scope": INVENTORY_SCOPE,
        "dependency_closure": DEPENDENCY_CLOSURE,
        "license": {
            "upstream_path": LEGAL_UPSTREAM_PATH,
            "retained_path": RETAINED_LEGAL_PATH.as_posix(),
            "git_blob_sha1": f"sha1:{release_legal_oid}",
            "sha256": f"sha256:{hashlib.sha256(release_legal).hexdigest()}",
        },
        "snapshots": [release, development],
    }
    return inventory, release_legal


def _canonical_json(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def _duplicate_checking_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise _DuplicateKeyError(f"duplicate object key {key!r}")
        value[key] = item
    return value


def _read_inventory(path):
    try:
        raw = path.read_bytes()
    except FileNotFoundError as exc:
        raise InventoryError(f"{INVENTORY_PATH.as_posix()}: file is missing") from exc
    except OSError as exc:
        raise InventoryError(f"{INVENTORY_PATH.as_posix()}: unable to read file") from exc
    try:
        value = json.loads(
            raw.decode("utf-8"), object_pairs_hook=_duplicate_checking_object
        )
    except UnicodeDecodeError as exc:
        raise InventoryError(
            f"{INVENTORY_PATH.as_posix()}: file is not UTF-8"
        ) from exc
    except _DuplicateKeyError as exc:
        raise InventoryError(f"{INVENTORY_PATH.as_posix()}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise InventoryError(
            f"{INVENTORY_PATH.as_posix()}: invalid JSON at line {exc.lineno}, "
            f"column {exc.colno}"
        ) from exc
    return raw, value


def _require_object(value, keys, label):
    if not isinstance(value, dict):
        raise InventoryError(f"{label}: must be an object")
    if tuple(value) != keys:
        raise InventoryError(
            f"{label}: keys must appear exactly in this order: {', '.join(keys)}"
        )


def _require_nonnegative_integer(value, label):
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise InventoryError(f"{label}: must be a nonnegative integer")


def _require_hash(value, pattern, label):
    if not isinstance(value, str) or pattern.fullmatch(value) is None:
        prefix = "sha1:<40 lowercase hex>" if pattern is SHA1_RE else "sha256:<64 lowercase hex>"
        raise InventoryError(f"{label}: must match {prefix}")


def _validate_file_rows(files, label):
    if not isinstance(files, list):
        raise InventoryError(f"{label}: must be an array")
    seen = set()
    paths = []
    for index, row in enumerate(files):
        row_label = f"{label}[{index}]"
        _require_object(row, FILE_KEYS, row_label)
        path = row.get("path")
        problem = _safe_source_path(path)
        if problem is not None:
            raise InventoryError(f"{row_label}.path: {problem}")
        if path in seen:
            raise InventoryError(f"{row_label}.path: duplicate {path!r}")
        seen.add(path)
        paths.append(path)
        if row.get("mode") != "100644":
            raise InventoryError(f"{row_label}.mode: must be '100644'")
        _require_nonnegative_integer(row.get("bytes"), f"{row_label}.bytes")
        _require_hash(row.get("git_blob_sha1"), SHA1_RE, f"{row_label}.git_blob_sha1")
        _require_hash(row.get("sha256"), SHA256_RE, f"{row_label}.sha256")
    if paths != sorted(paths):
        raise InventoryError(f"{label}: paths must be lexicographically ordered")


def _validate_snapshot(snapshot, index, expected_role, expected_revision):
    label = f"{INVENTORY_PATH.as_posix()}: snapshots[{index}]"
    _require_object(snapshot, SNAPSHOT_KEYS, label)
    if snapshot.get("role") != expected_role:
        raise InventoryError(f"{label}.role: must be {expected_role!r}")
    if snapshot.get("revision") != expected_revision:
        raise InventoryError(
            f"{label}.revision: must equal {expected_role} source pin"
        )
    _require_hash(snapshot.get("root_tree_sha1"), SHA1_RE, f"{label}.root_tree_sha1")
    for key in (
        "file_count",
        "total_bytes",
        "modelica_file_count",
        "package_order_count",
    ):
        _require_nonnegative_integer(snapshot.get(key), f"{label}.{key}")
    files = snapshot.get("files")
    _validate_file_rows(files, f"{label}.files")
    expected_counts = {
        "file_count": len(files),
        "total_bytes": sum(row["bytes"] for row in files),
        "modelica_file_count": sum(row["path"].endswith(".mo") for row in files),
        "package_order_count": sum(
            row["path"].endswith("/package.order") for row in files
        ),
    }
    for key, expected in expected_counts.items():
        if snapshot.get(key) != expected:
            raise InventoryError(f"{label}.{key}: must equal {expected}")


def _validate_inventory(value, release_revision, development_revision):
    label = INVENTORY_PATH.as_posix()
    _require_object(value, TOP_LEVEL_KEYS, label)
    constants = {
        "schema": SCHEMA,
        "repository": REPOSITORY,
        "source_root": SOURCE_ROOT,
        "inventory_scope": INVENTORY_SCOPE,
        "dependency_closure": DEPENDENCY_CLOSURE,
    }
    for key, expected in constants.items():
        if value.get(key) != expected:
            raise InventoryError(f"{label}: {key} must be {expected!r}")

    license_value = value.get("license")
    _require_object(license_value, LICENSE_KEYS, f"{label}: license")
    if license_value.get("upstream_path") != LEGAL_UPSTREAM_PATH:
        raise InventoryError(
            f"{label}: license.upstream_path must be {LEGAL_UPSTREAM_PATH!r}"
        )
    if license_value.get("retained_path") != RETAINED_LEGAL_PATH.as_posix():
        raise InventoryError(
            f"{label}: license.retained_path must be {RETAINED_LEGAL_PATH.as_posix()!r}"
        )
    _require_hash(
        license_value.get("git_blob_sha1"), SHA1_RE, f"{label}: license.git_blob_sha1"
    )
    _require_hash(
        license_value.get("sha256"), SHA256_RE, f"{label}: license.sha256"
    )

    snapshots = value.get("snapshots")
    if not isinstance(snapshots, list) or len(snapshots) != 2:
        raise InventoryError(f"{label}: snapshots must contain release then development")
    _validate_snapshot(snapshots[0], 0, "release", release_revision)
    _validate_snapshot(snapshots[1], 1, "development", development_revision)


def _first_difference(actual, expected, label=INVENTORY_PATH.as_posix()):
    if type(actual) is not type(expected):
        return f"{label}: generated value has a different type"
    if isinstance(expected, dict):
        for key in expected:
            difference = _first_difference(actual[key], expected[key], f"{label}.{key}")
            if difference is not None:
                return difference
        return None
    if isinstance(expected, list):
        if len(actual) != len(expected):
            return f"{label}: generated array length must be {len(expected)}"
        for index, expected_item in enumerate(expected):
            difference = _first_difference(
                actual[index], expected_item, f"{label}[{index}]"
            )
            if difference is not None:
                return difference
        return None
    if actual != expected:
        return f"{label}: value does not match the pinned Git objects"
    return None


def _stage_output(path, content):
    try:
        if path.read_bytes() == content:
            return None
    except FileNotFoundError:
        pass
    except OSError as exc:
        raise InventoryError(f"{path}: unable to read existing output") from exc
    temporary_name = None
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.", dir=path.parent
        )
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary_name, 0o644)
    except OSError as exc:
        if temporary_name is not None:
            try:
                Path(temporary_name).unlink()
            except FileNotFoundError:
                pass
        raise InventoryError(f"{path}: unable to stage output") from exc
    return Path(temporary_name)


def _write_outputs(outputs):
    staged = []
    try:
        for path, content in outputs:
            temporary_path = _stage_output(path, content)
            if temporary_path is not None:
                staged.append((temporary_path, path))
        for temporary_path, path in staged:
            os.replace(temporary_path, path)
    except OSError as exc:
        raise InventoryError("unable to replace generated output") from exc
    finally:
        for temporary_path, _ in staged:
            try:
                temporary_path.unlink()
            except FileNotFoundError:
                pass


def run(mode, release_root, development_root, repo_root=REPO_ROOT):
    """Generate expected bytes, then write or compare both governed artifacts."""
    repo_root = Path(repo_root).resolve()
    inventory, legal_bytes = _build_inventory(
        repo_root, Path(release_root), Path(development_root)
    )
    inventory_bytes = _canonical_json(inventory)
    inventory_path = repo_root / INVENTORY_PATH
    retained_legal_path = repo_root / RETAINED_LEGAL_PATH

    if mode == "write":
        _write_outputs(
            (
                (inventory_path, inventory_bytes),
                (retained_legal_path, legal_bytes),
            )
        )
    elif mode == "check":
        actual_bytes, actual_inventory = _read_inventory(inventory_path)
        release_revision = _read_pin(repo_root, RELEASE_PIN_PATH)
        development_revision = _read_pin(repo_root, DEVELOPMENT_PIN_PATH)
        _validate_inventory(
            actual_inventory, release_revision, development_revision
        )
        difference = _first_difference(actual_inventory, inventory)
        if difference is not None:
            raise InventoryError(difference)
        if actual_bytes != inventory_bytes:
            raise InventoryError(
                f"{INVENTORY_PATH.as_posix()}: bytes are not canonical two-space JSON "
                "with a final newline"
            )
        try:
            actual_legal = retained_legal_path.read_bytes()
        except FileNotFoundError as exc:
            raise InventoryError(
                f"{RETAINED_LEGAL_PATH.as_posix()}: file is missing"
            ) from exc
        except OSError as exc:
            raise InventoryError(
                f"{RETAINED_LEGAL_PATH.as_posix()}: unable to read file"
            ) from exc
        if actual_legal != legal_bytes:
            raise InventoryError(
                f"{RETAINED_LEGAL_PATH.as_posix()}: bytes do not match the pinned legal notice"
            )
    else:
        raise InventoryError("mode must be 'write' or 'check'")
    return inventory


def _parser():
    parser = argparse.ArgumentParser(
        description="Generate or check the pinned G36 Git-tree inventory."
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_const", const="write", dest="mode")
    mode.add_argument("--check", action="store_const", const="check", dest="mode")
    parser.add_argument("--release-root", required=True, type=Path)
    parser.add_argument("--development-root", required=True, type=Path)
    return parser


def main(argv=None, repo_root=REPO_ROOT):
    args = _parser().parse_args(argv)
    try:
        inventory = run(
            args.mode,
            args.release_root,
            args.development_root,
            repo_root=repo_root,
        )
    except InventoryError as exc:
        print(f"g36 source inventory: {exc}", file=sys.stderr)
        return 1
    release, development = inventory["snapshots"]
    verb = "wrote" if args.mode == "write" else "checked"
    print(
        f"g36 source inventory: {verb} {release['file_count']} release files and "
        f"{development['file_count']} development files OK"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
