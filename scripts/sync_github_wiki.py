#!/usr/bin/env python3
"""Transform docs/ into GitHub Wiki pages (User-*, Developer-*, Home, _Sidebar)."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DEFAULT_OWNER = "Bitslix"
DEFAULT_REPO = "BLXCode"
DEFAULT_BRANCH = "main"
DOCS_README = "docs/README.md"
HOME_PAGE = "Home"

LINK_RE = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")
IMG_SRC_RE = re.compile(r"""src=["']\.\./images/([^"']+)["']""")
MD_IMAGE_LINK_RE = re.compile(
    r"\[([^\]]*)\]\(\.\./images/([^)]+)\)"
)
IMAGE_SUFFIXES = (".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg")


def stem_to_title(stem: str) -> str:
    return "-".join(part.capitalize() for part in stem.split("-"))


def wiki_page_name(prefix: str, stem: str) -> str:
    return f"{prefix}-{stem_to_title(stem)}"


def build_doc_mapping(docs_root: Path) -> dict[Path, str]:
    """Map resolved docs/*.md paths to wiki page names."""
    mapping: dict[Path, str] = {}
    for subdir, prefix in (("user", "User"), ("developer", "Developer")):
        folder = docs_root / subdir
        if not folder.is_dir():
            continue
        for path in sorted(folder.glob("*.md")):
            mapping[path.resolve()] = wiki_page_name(prefix, path.stem)
    readme = (docs_root / "README.md").resolve()
    mapping[readme] = HOME_PAGE
    return mapping


def build_rel_mapping(repo_root: Path, doc_mapping: dict[Path, str]) -> dict[str, str]:
    """Relative posix path (from repo root) -> wiki page name."""
    out: dict[str, str] = {}
    for path, name in doc_mapping.items():
        try:
            rel = path.relative_to(repo_root.resolve())
        except ValueError:
            continue
        out[rel.as_posix()] = name
    return out


def raw_image_url(owner: str, repo: str, branch: str, filename: str) -> str:
    return (
        f"https://raw.githubusercontent.com/{owner}/{repo}/{branch}"
        f"/docs/images/{filename}"
    )


def blob_url(owner: str, repo: str, branch: str, rel_posix: str) -> str:
    return f"https://github.com/{owner}/{repo}/blob/{branch}/{rel_posix}"


def resolve_doc_path(from_file: Path, target: str, repo_root: Path) -> Path | None:
    """Resolve a markdown link target to a file under the repo."""
    path_part = target.split("#", 1)[0].strip()
    if not path_part or path_part.startswith(("http://", "https://", "mailto:")):
        return None
    if path_part.startswith("#"):
        return None
    resolved = (from_file.parent / path_part).resolve()
    if resolved.is_file():
        return resolved
    if not path_part.endswith(".md"):
        with_md = (from_file.parent / f"{path_part}.md").resolve()
        if with_md.is_file():
            return with_md
    return None


def wiki_link(page: str, anchor: str = "", label: str | None = None) -> str:
    if anchor and label is not None:
        return f"[{label}]({page}{anchor})"
    if anchor:
        return f"[{page}]({page}{anchor})"
    if label is not None and label.replace(" ", "-").lower() != page.lower():
        return f"[{label}]({page})"
    return f"[[{page}]]"


def transform_content(
    text: str,
    from_file: Path,
    repo_root: Path,
    rel_mapping: dict[str, str],
    owner: str,
    repo: str,
    branch: str,
) -> str:
    def replace_md_link(match: re.Match[str]) -> str:
        label, raw_target = match.group(1), match.group(2).strip()
        if raw_target.startswith(("http://", "https://", "mailto:", "#")):
            return match.group(0)

        anchor = ""
        target = raw_target
        if "#" in target:
            target, frag = target.split("#", 1)
            anchor = f"#{frag}"

        if not target:
            return match.group(0)

        resolved = resolve_doc_path(from_file, target if target else raw_target, repo_root)
        if resolved is None and target:
            resolved = resolve_doc_path(from_file, raw_target, repo_root)

        if resolved is not None:
            try:
                rel = resolved.relative_to(repo_root.resolve()).as_posix()
            except ValueError:
                return match.group(0)
            page = rel_mapping.get(rel)
            if page:
                return wiki_link(page, anchor, label)

        # Repo-relative paths outside docs/ (scripts, workflows, images, …)
        if target.startswith(("../", "../../")):
            path_part = target.split("#", 1)[0]
            blob_path = (from_file.parent / path_part).resolve()
            try:
                rel = blob_path.relative_to(repo_root.resolve()).as_posix()
            except ValueError:
                return match.group(0)
            if blob_path.is_file() or not path_part.endswith(".md"):
                if rel.startswith("docs/images/") and rel.lower().endswith(
                    IMAGE_SUFFIXES
                ):
                    name = Path(rel).name
                    url = raw_image_url(owner, repo, branch, name) + anchor
                else:
                    url = blob_url(owner, repo, branch, rel) + anchor
                return f"[{label}]({url})"

        return match.group(0)

    text = LINK_RE.sub(replace_md_link, text)

    def replace_img_src(match: re.Match[str]) -> str:
        name = match.group(1)
        url = raw_image_url(owner, repo, branch, name)
        quote = '"' if match.group(0).startswith('src="') else "'"
        return f"src={quote}{url}{quote}"

    text = IMG_SRC_RE.sub(replace_img_src, text)

    def replace_md_image_link(match: re.Match[str]) -> str:
        label, name = match.group(1), match.group(2).split("#", 1)[0]
        url = raw_image_url(owner, repo, branch, name)
        return f"[{label}]({url})"

    text = MD_IMAGE_LINK_RE.sub(replace_md_image_link, text)

    return text


def generate_sidebar(rel_mapping: dict[str, str], owner: str, repo: str, branch: str) -> str:
    user_pages: list[tuple[str, str]] = []
    dev_pages: list[tuple[str, str]] = []
    for rel, page in sorted(rel_mapping.items()):
        if rel.startswith("docs/user/"):
            user_pages.append((rel, page))
        elif rel.startswith("docs/developer/"):
            dev_pages.append((rel, page))

    lines = [
        "### User guides",
        "",
    ]
    for _rel, page in user_pages:
        lines.append(f"- [[{page}]]")
    lines.extend(["", "### Developer guides", ""])
    for _rel, page in dev_pages:
        lines.append(f"- [[{page}]]")
    lines.extend(
        [
            "",
            "---",
            "",
            f"[Documentation source]({blob_url(owner, repo, branch, 'docs/README.md')})",
            f" · [Repository](https://github.com/{owner}/{repo})",
            "",
        ]
    )
    return "\n".join(lines)


def collect_sources(docs_root: Path) -> list[tuple[Path, str]]:
    """(source path, output wiki filename without .md)."""
    out: list[tuple[Path, str]] = []
    readme = docs_root / "README.md"
    if readme.is_file():
        out.append((readme, HOME_PAGE))
    for subdir, prefix in (("user", "User"), ("developer", "Developer")):
        folder = docs_root / subdir
        if not folder.is_dir():
            continue
        for path in sorted(folder.glob("*.md")):
            out.append((path, wiki_page_name(prefix, path.stem)))
    return out


def sync_wiki_tree(
    repo_root: Path,
    output_dir: Path,
    owner: str,
    repo: str,
    branch: str,
) -> list[str]:
    docs_root = repo_root / "docs"
    if not docs_root.is_dir():
        raise SystemExit(f"Missing docs directory: {docs_root}")

    doc_mapping = build_doc_mapping(docs_root)
    rel_mapping = build_rel_mapping(repo_root, doc_mapping)

    output_dir.mkdir(parents=True, exist_ok=True)
    written: list[str] = []

    for src, page_name in collect_sources(docs_root):
        content = src.read_text(encoding="utf-8")
        transformed = transform_content(
            content, src.resolve(), repo_root, rel_mapping, owner, repo, branch
        )
        out_path = output_dir / f"{page_name}.md"
        out_path.write_text(transformed, encoding="utf-8")
        written.append(out_path.name)

    sidebar = generate_sidebar(rel_mapping, owner, repo, branch)
    sidebar_path = output_dir / "_Sidebar.md"
    sidebar_path.write_text(sidebar, encoding="utf-8")
    written.append(sidebar_path.name)

    # Remove stale generated pages (CI-only policy: full replace)
    allowed = {f"{p}.md" for _, p in collect_sources(docs_root)} | {"_Sidebar.md"}
    for existing in output_dir.glob("*.md"):
        if existing.name not in allowed:
            existing.unlink()
            written.append(f"(removed {existing.name})")

    return written


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="BLXCode repository root",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        required=True,
        help="Directory to write wiki *.md files",
    )
    parser.add_argument("--owner", default=DEFAULT_OWNER)
    parser.add_argument("--repo", default=DEFAULT_REPO)
    parser.add_argument("--branch", default=DEFAULT_BRANCH)
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print files that would be written, do not write",
    )
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    sources = collect_sources(repo_root / "docs")
    if args.dry_run:
        print(f"Would write {len(sources) + 1} pages to {args.output_dir}:")
        for _src, name in sources:
            print(f"  {name}.md")
        print("  _Sidebar.md")
        return 0

    written = sync_wiki_tree(
        repo_root, args.output_dir, args.owner, args.repo, args.branch
    )
    print(f"Wrote {len(written)} file(s) to {args.output_dir}")
    for name in sorted(w for w in written if not w.startswith("(removed")):
        if not name.startswith("(removed"):
            print(f"  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
