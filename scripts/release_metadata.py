"""Validate release metadata and prepare the curated part of release notes."""

import argparse
import datetime
from pathlib import Path
import re
import tomllib


VERSION = (
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-(?:alpha|beta|rc)\.(?:0|[1-9][0-9]*))?"
)


def changelog_summary(changelog: str, section: str, dated: bool) -> str:
    suffix = r" - ([0-9]{4}-[0-9]{2}-[0-9]{2})" if dated else ""
    pattern = rf"^## \[{re.escape(section)}\]{suffix}[^\S\n]*\n(.*?)(?=^## |\Z)"
    matches = list(re.finditer(pattern, changelog, re.M | re.S))
    if len(matches) != 1:
        raise ValueError(
            "Expected one changelog heading: ## [version] - YYYY-MM-DD "
            "(or ## [Unreleased] for previews)."
        )
    match = matches[0]
    if dated:
        datetime.date.fromisoformat(match[1])
    return match[2 if dated else 1].strip()


def prepare(root: Path, tag: str | None = None) -> tuple[str, str]:
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    package = manifest["package"]
    version = package["version"]
    if not re.fullmatch(VERSION, version):
        raise ValueError("Use X.Y.Z or X.Y.Z-alpha.N, -beta.N, or -rc.N versions.")
    if tag is not None and tag != f"v{version}":
        raise ValueError("The tag must exactly match v<Cargo.toml version>.")
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    locked = [p["version"] for p in lock["package"] if p["name"] == package["name"]]
    if locked != [version]:
        raise ValueError("Cargo.lock and Cargo.toml package versions must match.")

    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    summary = changelog_summary(changelog, version, True) if tag else changelog_summary(changelog, "Unreleased", False)
    if not tag and not summary:
        summary = changelog_summary(changelog, version, True)
    if not summary:
        raise ValueError("The changelog section must contain a release summary.")
    notes = (
        f"## Highlights\n\n{summary}\n\n"
        "## Downloads\n\n"
        f"- Windows x64: `lumen-{version}-windows-x64.zip` (portable; extract before running).\n"
        f"- Matching source and dependency sources: `lumen-{version}-source.zip`.\n"
        "- `SHA256SUMS` verifies both ZIP archives.\n\n"
        "Lumen is licensed under GPL-3.0-only. When redistributing a binary, provide "
        "equivalent access to its matching source archive. See the bundled notices.\n"
    )
    return version, notes


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", help="Validate a release tag; omit to preview Unreleased notes.")
    parser.add_argument("--output", type=Path, default=Path("target/release-notes.md"))
    args = parser.parse_args()
    try:
        version, notes = prepare(Path(__file__).resolve().parent.parent, args.tag)
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"Release validation failed: {error}\n")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(notes, encoding="utf-8", newline="\n")
    print(f"Validated Lumen {version}; notes: {args.output}")


if __name__ == "__main__":
    main()
