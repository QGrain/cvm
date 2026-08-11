import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import update_remote_index as remote_index  # noqa: E402


def sample_index() -> dict[str, object]:
    return {
        "schema_version": 1,
        "generated_at": "2026-08-01T00:00:00Z",
        "cvm": {"latest": "v0.1.1"},
        "compilers": {
            "gcc": [
                {
                    "version": "15.1.0",
                    "date": "2025-04-25",
                    "url": remote_index.gcc_url("15.1.0"),
                },
                {
                    "version": "14.2.0",
                    "date": "2024-08-01",
                    "url": remote_index.gcc_url("14.2.0"),
                },
            ],
            "llvm": [
                {
                    "version": "21.1.8",
                    "date": "2026-01-07",
                    "url": remote_index.llvm_url("21.1.8"),
                },
                {
                    "version": "20.1.8",
                    "date": "2025-07-12",
                    "url": remote_index.llvm_url("20.1.8"),
                },
            ],
        },
    }


class RemoteIndexUpdateTests(unittest.TestCase):
    def test_unchanged_index_preserves_existing_timestamp_and_file(self) -> None:
        existing = sample_index()
        updated = copy.deepcopy(existing)
        updated["generated_at"] = "2026-09-01T00:00:00Z"

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "remote-index.json"
            original = json.dumps(existing, indent=2) + "\n"
            output.write_text(original)

            self.assertFalse(remote_index.write_index_if_changed(output, updated))
            self.assertEqual(output.read_text(), original)

    def test_update_rejects_missing_historical_versions(self) -> None:
        existing = sample_index()
        updated = copy.deepcopy(existing)
        updated["compilers"]["gcc"].pop()

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "remote-index.json"
            output.write_text(json.dumps(existing))

            with self.assertRaisesRegex(ValueError, "would remove gcc versions: 14.2.0"):
                remote_index.write_index_if_changed(output, updated)

    def test_update_writes_new_release_and_timestamp(self) -> None:
        existing = sample_index()
        updated = copy.deepcopy(existing)
        updated["generated_at"] = "2026-09-01T00:00:00Z"
        updated["compilers"]["gcc"].insert(
            0,
            {
                "version": "16.1.0",
                "date": "2026-04-30",
                "url": remote_index.gcc_url("16.1.0"),
            },
        )

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "remote-index.json"
            output.write_text(json.dumps(existing))

            self.assertTrue(remote_index.write_index_if_changed(output, updated))
            self.assertEqual(json.loads(output.read_text()), updated)

    def test_update_rejects_rewritten_historical_entries(self) -> None:
        existing = sample_index()
        updated = copy.deepcopy(existing)
        updated["compilers"]["llvm"][0]["date"] = "2026-01-08"

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "remote-index.json"
            output.write_text(json.dumps(existing))

            with self.assertRaisesRegex(ValueError, "would rewrite existing llvm entries: 21.1.8"):
                remote_index.write_index_if_changed(output, updated)


if __name__ == "__main__":
    unittest.main()
