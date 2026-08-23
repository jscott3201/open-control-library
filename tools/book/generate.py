#!/usr/bin/env python3
"""Generate the mdBook source tree from the repository's canonical files.

Single source of truth stays in faults/, playbooks/, clusters/, points/ and
SCHEMA.md — this script derives book/src/ from them at build time and is run
by CI before `mdbook build`. Nothing under book/src/ is ever edited by hand.

Usage: python3 tools/book/generate.py   (from the repo root or anywhere)
"""

import json
import re
import shutil
import sys
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parents[2]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

from tools.point_resolution import load_point_corpus  # noqa: E402


SRC = REPO / "book" / "src"

FRONTMATTER_RE = re.compile(r"\A---\n(.*?)\n---\n", re.DOTALL)


def read_card(card_path: Path):
    text = card_path.read_text(encoding="utf-8")
    m = FRONTMATTER_RE.match(text)
    if not m:
        sys.exit(f"{card_path}: no YAML frontmatter")
    fm = yaml.safe_load(m.group(1))
    body = text[m.end():]
    return fm, body


SVG_ROOT_RE = re.compile(r"<svg\b[^>]*>")
SVG_VIEWBOX_RE = re.compile(
    r'viewBox="\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)[\s,]+([\d.]+)\s*"'
)


def copy_svg(src: Path, dst: Path):
    """Copy an SVG, injecting width/height from the viewBox when absent.

    An <svg> with only a viewBox has no intrinsic size inside an <img>
    element. The book wraps diagrams in shrink-to-fit self-links, and the
    max-width:100% / shrink-to-fit circularity collapses intrinsic-size-less
    images to 0x0 — they load but render invisible. Explicit pixel
    attributes restore the intrinsic size; book/custom.css scales them back
    down responsively.
    """
    text = src.read_text(encoding="utf-8")
    m = SVG_ROOT_RE.search(text)
    if m and not re.search(r'(?<![-\w])(width|height)\s*=', m.group(0)):
        vb = SVG_VIEWBOX_RE.search(m.group(0))
        if vb:
            w, h = vb.group(1), vb.group(2)
            text = (text[: m.end() - 1]
                    + f' width="{w}" height="{h}">'
                    + text[m.end():])
    dst.write_text(text, encoding="utf-8")


def fault_link(fid: str, from_depth: int, known: set) -> str:
    """Markdown link to a fault page if it exists in this build, else plain text."""
    if fid in known:
        prefix = "../" * from_depth
        family = fid.split("-")[0].lower()
        return f"[{fid}]({prefix}faults/{family}/{fid}.md)"
    return fid


def yaml_list(v):
    return v if isinstance(v, list) else ([] if v is None else [v])


def render_fault_page(
    fid: str, fdir: Path, fm: dict, body: str, known: set, point_corpus
) -> str:
    out = [f"# {fid} — {fm.get('name', '')}\n"]

    status = fm.get("status", "")
    verified = fm.get("verified") or {}
    if status == "verified" and verified:
        status = (
            f"**verified** — engine `{verified.get('engine_rev')}`, "
            f"`{verified.get('content_id')}`, {verified.get('date')}"
        )
    rows = [
        ("Status", status),
        ("Severity", str(fm.get("severity", ""))),
        ("Method", fm.get("method", "")),
        ("Phase", str(fm.get("phase", ""))),
        ("Category", fm.get("category", "")),
        ("Confidence", fm.get("confidence", "")),
        ("Estimation", fm.get("estimation_method", "")),
        ("G36", fm.get("g36") or "—"),
        ("Clusters", ", ".join(yaml_list(fm.get("clusters"))) or "—"),
        ("Suppresses", ", ".join(fault_link(x, 2, known) for x in yaml_list(fm.get("suppresses"))) or "—"),
        ("Suppressed by", ", ".join(fault_link(x, 2, known) for x in yaml_list(fm.get("suppressed_by"))) or "—"),
        ("Related", ", ".join(fault_link(x, 2, known) for x in yaml_list(fm.get("related"))) or "—"),
        ("Playbooks", ", ".join(f"[{p}](../../playbooks/{p}.md)" for p in yaml_list(fm.get("playbooks"))) or "—"),
        ("Source", "; ".join(yaml_list(fm.get("source")))),
        ("Operating states", fm.get("operating_states", "")),
    ]
    out.append("| | |\n|---|---|")
    for k, v in rows:
        out.append(f"| **{k}** | {v} |")
    out.append("")

    if fm.get("preconditions"):
        out.append(f"> **Preconditions (host-enforced):** {fm['preconditions']}\n")

    pts = yaml_list(fm.get("points"))
    if pts:
        family = fid.split("-")[0].lower()
        dictionary_path = f"points/{family}.points.json"
        links = []
        for point in pts:
            resolved = point_corpus.resolve_bare(dictionary_path, point)
            canonical_family = Path(resolved.path).name.removesuffix(".points.json")
            links.append(
                f"[`{point}`](../../points/{canonical_family}.md#{resolved.name})"
            )
        linked = ", ".join(links)
        out.append(f"**Points:** {linked}\n")

    outs = yaml_list(fm.get("outputs"))
    if outs:
        out.append("**Outputs:**\n")
        for o in outs:
            out.append(f"- `{o.get('name')}` — {o.get('description', '')}")
        out.append("")

    params = fm.get("params") or {}
    if params:
        out.append("**Parameters:**\n")
        out.append("| Name | Default | Unit | CXF path | Description |\n|---|---|---|---|---|")
        for name, spec in params.items():
            cxf = spec.get("cxf")
            cxf = ", ".join(f"`{c}`" for c in (cxf if isinstance(cxf, list) else [cxf]))
            out.append(
                f"| `{name}` | {spec.get('default')} | {spec.get('unit', '')} "
                f"| {cxf} | {spec.get('description', '')} |"
            )
        out.append("")

    # Body: fix diagram and playbook links for the book layout. Diagram
    # images become links to themselves so wide graphs open full-size.
    body = re.sub(r"!\[([^\]]*)\]\(diagram\.svg\)",
                  rf"[![\1]({fid}.svg)]({fid}.svg)", body)
    body = body.replace("](diagram.svg)", f"]({fid}.svg)")
    body = body.replace("](../../../playbooks/", "](../../playbooks/")
    out.append(body.rstrip() + "\n")

    vectors_path = fdir / "vectors.json"
    if vectors_path.is_file():
        vec = json.loads(vectors_path.read_text(encoding="utf-8"))
        scenarios = vec.get("scenarios", [])
        clock = vec.get("clock", {})
        out.append(
            f"\n## Test Vectors\n\n{len(scenarios)} scenarios, "
            f"clock step {clock.get('step_s')} s over {clock.get('horizon_s')} s.\n"
        )
        out.append("| Scenario | Description |\n|---|---|")
        for s in scenarios:
            out.append(f"| `{s.get('name')}` | {s.get('description', '')} |")
        out.append("\n<details><summary>vectors.json</summary>\n")
        out.append("```json")
        out.append(json.dumps(vec, indent=2))
        out.append("```\n</details>")

    return "\n".join(out) + "\n"


def build_family(family_dir: Path, known: set, point_corpus):
    family = family_dir.name
    dest = SRC / "faults" / family
    dest.mkdir(parents=True, exist_ok=True)

    entries = []
    for fdir in sorted(family_dir.iterdir()):
        if not (fdir / "card.md").is_file():
            continue
        fid = fdir.name
        fm, body = read_card(fdir / "card.md")
        (dest / f"{fid}.md").write_text(
            render_fault_page(fid, fdir, fm, body, known, point_corpus),
            encoding="utf-8",
        )
        svg = fdir / "diagram.svg"
        if svg.is_file():
            copy_svg(svg, dest / f"{fid}.svg")
        entries.append((fid, fm.get("name", "")))

    # Family index: the README with fault IDs linked to their pages.
    readme = family_dir / "README.md"
    if readme.is_file():
        text = readme.read_text(encoding="utf-8")
        for fid, _ in entries:
            text = text.replace(f"| {fid} |", f"| [{fid}]({fid}.md) |")
        text = text.replace("](../../points/", "](../../points/").replace(
            f"](../../points/{family}.points.json)", f"](../../points/{family}.md)"
        )
        (dest / "index.md").write_text(text, encoding="utf-8")
    return entries


def build_points(point_corpus, dest=None):
    dest = SRC / "points" if dest is None else Path(dest)
    dest.mkdir(parents=True, exist_ok=True)
    pages = []
    for dictionary_path in point_corpus.paths:
        dictionary = point_corpus.dictionaries[dictionary_path]
        family = Path(dictionary_path).name.removesuffix(".points.json")
        d = dictionary.document
        pts = d.get("points", [])
        out = [f"# Point Dictionary: {family.upper()}\n"]
        if d.get("notes"):
            out.append(f"{d['notes']}\n")
        out.append("| Point | Kind | Unit | Brick | Derived | Provisional |\n|---|---|---|---|---|---|")
        for p in pts:
            out.append(
                f"| [`{p['name']}`](#{p['name']}) | {p.get('kind')} | {p.get('unit')} "
                f"| {p.get('brick') or '—'} | {'✓' if p.get('derived') else ''} "
                f"| {'✓' if p.get('provisional') else ''} |"
            )
        out.append("")
        for p in pts:
            out.append(f'\n## {p["name"]} {{#{p["name"]}}}\n')
            out.append(p.get("description", "") + "\n")
            s223 = p.get("s223")
            if s223 and s223.get("pattern"):
                out.append(f"- **223P:** {s223['pattern']}")
            if p.get("qudt_unit"):
                out.append(f"- **QUDT unit:** `{p['qudt_unit']}`")
            if p.get("notes"):
                out.append(f"\n{p['notes']}")
        if dictionary.aliases:
            out.append("\n## Compatibility aliases\n")
            for name, _ in dictionary.aliases:
                resolved = point_corpus.resolve_bare(dictionary_path, name)
                canonical_family = Path(resolved.path).name.removesuffix(
                    ".points.json"
                )
                out.append(
                    f"- [`{name}`]({canonical_family}.md#{resolved.name}) → "
                    f"`{resolved.ref}`"
                )
        (dest / f"{family}.md").write_text("\n".join(out) + "\n", encoding="utf-8")
        pages.append(family)
    return pages


def build_clusters(known: set):
    d = json.loads((REPO / "clusters" / "clusters.json").read_text(encoding="utf-8"))
    clusters = d.get("clusters", d if isinstance(d, list) else [])
    out = ["# Fault Clusters\n"]
    out.append(
        "Clusters group faults that share a root cause. The **trigger** is the "
        "fault that usually fires first; members refine the diagnosis.\n"
    )
    for c in clusters:
        out.append(f"\n## {c.get('id')} — {c.get('name')}\n")
        out.append("| | |\n|---|---|")
        out.append(f"| **Trigger** | {fault_link(c.get('trigger', ''), 0, known)} |")
        members = ", ".join(fault_link(m, 0, known) for m in c.get("members", []))
        out.append(f"| **Members** | {members} |")
        if c.get("playbook"):
            pb = c["playbook"]
            if (REPO / "playbooks" / f"{pb}.md").is_file():
                out.append(f"| **Playbook** | [{pb}](playbooks/{pb}.md) |")
            else:
                out.append(f"| **Playbook** | {pb} *(planned)* |")
        if c.get("prevalence"):
            out.append(f"| **Prevalence** | {c['prevalence']} |")
        if c.get("energy_impact"):
            out.append(f"| **Energy impact** | {c['energy_impact']} |")
        if c.get("description"):
            out.append(f"\n{c['description']}")
    (SRC / "clusters.md").write_text("\n".join(out) + "\n", encoding="utf-8")


def build_playbooks():
    dest = SRC / "playbooks"
    dest.mkdir(parents=True, exist_ok=True)
    names = []
    for pfile in sorted((REPO / "playbooks").glob("*.md")):
        shutil.copy(pfile, dest / pfile.name)
        title = pfile.read_text(encoding="utf-8").splitlines()[0].lstrip("# ")
        names.append((pfile.stem, title))
    index = ["# Remediation Playbooks\n"]
    for stem, title in names:
        index.append(f"- [{title}]({stem}.md)")
    (dest / "index.md").write_text("\n".join(index) + "\n", encoding="utf-8")
    return names


def main():
    point_corpus = load_point_corpus(REPO).require_valid()
    if SRC.exists():
        shutil.rmtree(SRC)
    SRC.mkdir(parents=True)

    # Every fault directory that exists in this build gets a page; links to
    # faults outside this set stay plain text.
    known = {
        fdir.name
        for fam in sorted((REPO / "faults").iterdir())
        if fam.is_dir()
        for fdir in fam.iterdir()
        if (fdir / "card.md").is_file()
    }

    # Introduction and schema, links rewritten for the book layout.
    if (REPO / "assets").is_dir():
        shutil.copytree(REPO / "assets", SRC / "assets")
        for asset_svg in (SRC / "assets").rglob("*.svg"):
            copy_svg(asset_svg, asset_svg)
    intro = (REPO / "README.md").read_text(encoding="utf-8")
    intro = intro.replace("**`SCHEMA.md`**", "**[`SCHEMA.md`](schema.md)**")
    # Fault-dir asset links flatten in the book (<ID>/diagram.svg -> <ID>.svg)
    # and embed as self-links so wide graphs open full-size.
    intro = re.sub(r"!\[([^\]]*)\]\(faults/(\w+)/([A-Z]+-\d+)/diagram\.svg\)",
                   r"[![\1](faults/\2/\3.svg)](faults/\2/\3.svg)", intro)
    intro = re.sub(r"\(faults/(\w+)/([A-Z]+-\d+)/diagram\.svg\)",
                   r"(faults/\1/\2.svg)", intro)
    # License files are not book pages; point at the repository.
    for lic in ("LICENSE-APACHE", "LICENSE-MIT"):
        intro = intro.replace(
            f"]({lic})",
            f"](https://github.com/jscott3201/open-control-library/blob/main/{lic})")
    intro += (
        "\n---\n\n*This book is generated from the repository by "
        "`tools/book/generate.py`; the files above are the source of truth.*\n"
    )
    (SRC / "index.md").write_text(intro, encoding="utf-8")
    shutil.copy(REPO / "SCHEMA.md", SRC / "schema.md")

    families = {}
    for fam in sorted((REPO / "faults").iterdir()):
        if fam.is_dir():
            entries = build_family(fam, known, point_corpus)
            if entries:
                families[fam.name] = entries

    # Fault code map: the registry rendered as one library-wide table.
    registry = json.loads(
        (REPO / "faults" / "registry.json").read_text(encoding="utf-8")
    )

    # Legacy-URL redirect stubs: pre-renumbering pages redirect to the new
    # ids so external links keep working (mdBook copies non-md files from
    # src into the output verbatim).
    redirect_html = (
        '<!DOCTYPE html><html><head><meta charset="utf-8">\n'
        '<meta http-equiv="refresh" content="0; url={new}.html">\n'
        '<link rel="canonical" href="{new}.html">\n'
        '<title>{old} moved to {new}</title></head>\n'
        '<body>This rule is now <a href="{new}.html">{new}</a>.</body></html>\n'
    )
    aliases = {
        # the sys flatline/spike pair carried a brief intermediate id the
        # registry does not record (renamed twice on 2026-08-18)
        "SYS-FC-100": "SYS-0009",
        "SYS-FC-101": "SYS-0010",
    }
    for r in registry.get("rules", []):
        if r.get("legacy_id"):
            aliases[r["legacy_id"]] = r["id"]
    for old, new in aliases.items():
        fam = new.split("-")[0].lower()
        stub_dir = SRC / "faults" / fam
        if stub_dir.is_dir():
            (stub_dir / f"{old}.html").write_text(
                redirect_html.format(old=old, new=new), encoding="utf-8"
            )
    reg = ["# Fault Code Map\n",
           "One row per rule, from [`faults/registry.json`](https://github.com/"
           "jscott3201/open-control-library/blob/main/faults/registry.json). "
           "`Legacy ID` is the rule's pre-renumbering code.\n",
           "| ID | Name | Family | Method | Status | Legacy ID |",
           "|---|---|---|---|---|---|"]
    for r in registry.get("rules", []):
        fam = r["family"]
        reg.append(
            f"| [{r['id']}](faults/{fam}/{r['id']}.md) | {r['name']} | {fam.upper()} "
            f"| {r['method']} | {r['status']} | {r.get('legacy_id') or '—'} |"
        )
    (SRC / "registry.md").write_text("\n".join(reg) + "\n", encoding="utf-8")

    build_clusters(known)
    playbooks = build_playbooks()
    point_pages = build_points(point_corpus)

    summary = ["# Summary\n", "[Introduction](index.md)", "[Schema](schema.md)\n", "# Fault Rules\n"]
    for fam, entries in families.items():
        summary.append(f"- [{fam.upper()}](faults/{fam}/index.md)")
        for fid, name in entries:
            summary.append(f"  - [{fid} — {name}](faults/{fam}/{fid}.md)")
    summary.append("\n# Reference\n")
    summary.append("- [Fault Code Map](registry.md)")
    summary.append("- [Fault Clusters](clusters.md)")
    summary.append("- [Playbooks](playbooks/index.md)")
    for stem, title in playbooks:
        summary.append(f"  - [{title}](playbooks/{stem}.md)")
    for fam in point_pages:
        summary.append(f"- [Point Dictionary: {fam.upper()}](points/{fam}.md)")
    (SRC / "SUMMARY.md").write_text("\n".join(summary) + "\n", encoding="utf-8")

    n_pages = sum(len(e) for e in families.values())
    print(f"generated book/src: {n_pages} fault pages, {len(playbooks)} playbooks, "
          f"{len(point_pages)} point dictionaries")


if __name__ == "__main__":
    main()
