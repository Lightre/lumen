"""Release guards must reject mismatched versions and incomplete release notes."""

from pathlib import Path
import tempfile
import unittest

from release_metadata import prepare


class ReleaseMetadataTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.write("Cargo.toml", '[package]\nname = "lumen"\nversion = "0.1.0"\n')
        self.write("Cargo.lock", '[[package]]\nname = "lumen"\nversion = "0.1.0"\n')
        self.write("CHANGELOG.md", "# Changelog\n\n## [Unreleased]\n\n- Next change.\n\n## [0.1.0] - 2026-10-21\n\n- First version.\n\n## [0.0.1] - 2026-10-20\n\n- Older change.\n")

    def write(self, name, text):
        (self.root / name).write_text(text, encoding="utf-8")

    def test_release_selects_only_the_requested_section(self):
        version, notes = prepare(self.root, "v0.1.0")
        self.assertEqual(version, "0.1.0")
        self.assertIn("First version.", notes)
        self.assertNotIn("Next change.", notes)
        self.assertNotIn("Older change.", notes)
        self.assertIn("lumen-0.1.0-source.zip", notes)

    def test_preview_uses_unreleased(self):
        _, notes = prepare(self.root)
        self.assertIn("Next change.", notes)
        self.assertNotIn("First version.", notes)

    def test_mismatched_or_malformed_tags_are_rejected(self):
        for tag in ("v0.2.0", "0.1.0", "v0.1.0;echo bad"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                prepare(self.root, tag)

    def test_preview_uses_prepared_entry_when_unreleased_is_empty(self):
        self.write("CHANGELOG.md", "## [Unreleased]\n\n## [0.1.0] - 2026-10-21\n\n- Prepared version.\n")
        _, notes = prepare(self.root)
        self.assertIn("Prepared version.", notes)

    def test_preview_rejects_an_empty_changelog(self):
        self.write("CHANGELOG.md", "## [Unreleased]\n\n")
        with self.assertRaises(ValueError):
            prepare(self.root)

    def test_outdated_lockfile_is_rejected(self):
        self.write("Cargo.lock", '[[package]]\nname = "lumen"\nversion = "0.0.1"\n')
        with self.assertRaises(ValueError):
            prepare(self.root, "v0.1.0")

    def test_missing_empty_duplicate_and_invalid_dated_sections_are_rejected(self):
        for content in (
            "## [Unreleased]\n\n- Next change.\n",
            "## [0.1.0] - 2026-10-21\n\n",
            "## [0.1.0] - 2026-10-21\n\n- A.\n\n## [0.1.0] - 2026-10-21\n\n- B.\n",
            "## [0.1.0] - 2026-02-30\n\n- Invalid date.\n",
        ):
            with self.subTest(content=content):
                self.write("CHANGELOG.md", content)
                with self.assertRaises(ValueError):
                    prepare(self.root, "v0.1.0")

    def test_prerelease_version_matches_assets(self):
        self.write("Cargo.toml", '[package]\nname = "lumen"\nversion = "0.2.0-rc.1"\n')
        self.write("Cargo.lock", '[[package]]\nname = "lumen"\nversion = "0.2.0-rc.1"\n')
        self.write("CHANGELOG.md", "## [0.2.0-rc.1] - 2026-10-21\n\n- Candidate.\n")
        _, notes = prepare(self.root, "v0.2.0-rc.1")
        self.assertIn("lumen-0.2.0-rc.1-windows-x64.zip", notes)


if __name__ == "__main__":
    unittest.main()
