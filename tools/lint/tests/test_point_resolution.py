import json
import shutil
import socket
import tempfile
import unittest
import urllib.request
from pathlib import Path
from unittest import mock

from tools.point_resolution import PointResolutionError, load_point_corpus


PRODUCT_ROOT = Path(__file__).resolve().parents[3]
POINT_FILES = tuple(
    path.relative_to(PRODUCT_ROOT).as_posix()
    for path in sorted((PRODUCT_ROOT / "points").glob("*.points.json"))
)


class PointResolutionTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for relative_path in POINT_FILES:
            self.restore(relative_path)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def restore(self, relative_path):
        source = PRODUCT_ROOT / relative_path
        destination = self.root / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    def read_json(self, relative_path):
        return json.loads((self.root / relative_path).read_text(encoding="utf-8"))

    def write_json(self, relative_path, value):
        path = self.root / relative_path
        path.write_text(
            json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    def mutate(self, relative_path, change):
        value = self.read_json(relative_path)
        change(value)
        self.write_json(relative_path, value)

    def assert_invalid(self, expected):
        corpus = load_point_corpus(self.root)
        self.assertTrue(
            any(expected in error for error in corpus.errors),
            f"{expected!r} not found in {corpus.errors!r}",
        )
        self.assertEqual(corpus.errors, tuple(sorted(corpus.errors)))
        with self.assertRaises(PointResolutionError):
            corpus.resolve_bare("points/vav.points.json", "zone_temp")
        return corpus.errors

    def test_production_inventory_and_contextual_resolution(self):
        corpus = load_point_corpus(PRODUCT_ROOT).require_valid()
        concrete_count = sum(
            len(dictionary.points) for dictionary in corpus.dictionaries.values()
        )
        alias_count = sum(
            len(dictionary.aliases) for dictionary in corpus.dictionaries.values()
        )
        v2_paths = {
            path
            for path, document in corpus.documents.items()
            if document["schema"] == "cxf-library/points/v2"
        }
        self.assertEqual(len(corpus.paths), 15)
        self.assertEqual(concrete_count, 183)
        self.assertEqual(alias_count, 4)
        self.assertEqual(
            v2_paths,
            {
                "points/sys.points.json",
                "points/vav.points.json",
                "points/zone.points.json",
            },
        )

        expected = {
            ("points/vav.points.json", "zone_temp"): "points/zone.points.json#zone_temp",
            (
                "points/vav.points.json",
                "zone_temp_sp_htg",
            ): "points/zone.points.json#zone_temp_sp_htg",
            (
                "points/vav.points.json",
                "zone_temp_sp_clg",
            ): "points/zone.points.json#zone_temp_sp_clg",
            ("points/sys.points.json", "occ_sensor"): "points/zone.points.json#occ_sensor",
            (
                "points/zone.points.json",
                "zone_temp",
            ): "points/zone.points.json#zone_temp",
            (
                "points/vav.points.json",
                "zone_airflow",
            ): "points/vav.points.json#zone_airflow",
        }
        for (path, name), target in expected.items():
            with self.subTest(path=path, name=name):
                self.assertEqual(corpus.resolve_bare(path, name).ref, target)

        self.assertEqual(
            corpus.resolve_ref("points/vav.points.json#zone_temp").ref,
            "points/zone.points.json#zone_temp",
        )
        with self.assertRaisesRegex(PointResolutionError, "no local point or alias"):
            corpus.resolve_bare("points/sys.points.json", "zone_temp")
        with self.assertRaisesRegex(PointResolutionError, "malformed point reference"):
            corpus.resolve_ref("https://example.com/points/zone.points.json#zone_temp")

    def test_resolution_is_repeatable_and_offline(self):
        with mock.patch.object(
            socket, "socket", side_effect=AssertionError("network socket opened")
        ), mock.patch.object(
            urllib.request,
            "urlopen",
            side_effect=AssertionError("network URL opened"),
        ):
            first = load_point_corpus(self.root).require_valid()
            second = load_point_corpus(self.root).require_valid()
            refs_one = [
                first.resolve_bare("points/vav.points.json", name).ref
                for name in ("zone_temp", "zone_temp_sp_htg", "zone_temp_sp_clg")
            ]
            refs_two = [
                second.resolve_bare("points/vav.points.json", name).ref
                for name in ("zone_temp", "zone_temp_sp_htg", "zone_temp_sp_clg")
            ]
        self.assertEqual(first.errors, second.errors)
        self.assertEqual(refs_one, refs_two)

    def test_v1_rejects_imports_and_aliases(self):
        for key in ("imports", "aliases"):
            with self.subTest(key=key):
                self.mutate("points/ahu.points.json", lambda value, k=key: value.update({k: []}))
                self.assert_invalid(f"points/ahu.points.json: unexpected key {key!r}")
                self.restore("points/ahu.points.json")

    def test_schema_requires_a_supported_string_without_traceback(self):
        expected = "points/ahu.points.json: schema must be one of"
        for schema in ([], {}):
            with self.subTest(schema=schema):
                self.mutate(
                    "points/ahu.points.json",
                    lambda value, bad_schema=schema: value.update(schema=bad_schema),
                )
                errors = self.assert_invalid(expected)
                self.assertFalse(any("Traceback" in error for error in errors))
                self.restore("points/ahu.points.json")

    def test_v2_requires_array_imports_and_aliases(self):
        for key in ("imports", "aliases"):
            with self.subTest(key=key, case="missing"):
                self.mutate("points/zone.points.json", lambda value, k=key: value.pop(k))
                self.assert_invalid(f"points/zone.points.json: missing required key {key!r}")
                self.restore("points/zone.points.json")
            with self.subTest(key=key, case="wrong type"):
                self.mutate(
                    "points/zone.points.json", lambda value, k=key: value.update({k: {}})
                )
                self.assert_invalid(f"points/zone.points.json: {key} must be an array")
                self.restore("points/zone.points.json")

    def test_import_paths_duplicates_targets_and_cycles_fail(self):
        self.mutate(
            "points/vav.points.json",
            lambda value: value.update(
                imports=["points/zone.points.json", "points/zone.points.json"]
            ),
        )
        self.assert_invalid("imports[1]: duplicate 'points/zone.points.json'")
        self.restore("points/vav.points.json")

        for target in (
            "/points/zone.points.json",
            "points/../zone.points.json",
            "https://example.com/zone.points.json",
        ):
            with self.subTest(target=target):
                self.mutate(
                    "points/zone.points.json",
                    lambda value, path=target: value.update(imports=[path]),
                )
                self.assert_invalid("must be a root-relative points/<family>.points.json path")
                self.restore("points/zone.points.json")

        self.mutate(
            "points/zone.points.json",
            lambda value: value.update(imports=["points/zone.points.json"]),
        )
        self.assert_invalid("imports[0]: self-import is forbidden")
        self.restore("points/zone.points.json")

        self.mutate(
            "points/zone.points.json",
            lambda value: value.update(imports=["points/missing.points.json"]),
        )
        self.assert_invalid("target 'points/missing.points.json' is missing")
        self.restore("points/zone.points.json")

        self.mutate(
            "points/zone.points.json",
            lambda value: value.update(imports=["points/vav.points.json"]),
        )
        self.assert_invalid(
            "points: import cycle: points/vav.points.json -> points/zone.points.json -> points/vav.points.json"
        )

    def test_alias_shape_uniqueness_collision_and_targets_fail_closed(self):
        self.mutate(
            "points/vav.points.json",
            lambda value: value["aliases"].append(dict(value["aliases"][0])),
        )
        self.assert_invalid("aliases[3].name: duplicate 'zone_temp'")
        self.restore("points/vav.points.json")

        self.mutate(
            "points/vav.points.json",
            lambda value: value["aliases"][0].update(name="zone_airflow"),
        )
        self.assert_invalid("collides with local point 'zone_airflow'")
        self.restore("points/vav.points.json")

        self.mutate(
            "points/vav.points.json",
            lambda value: value["aliases"][0].update(extra=True),
        )
        self.assert_invalid("aliases[0]: unexpected key 'extra'")
        self.restore("points/vav.points.json")

        for target in (
            "points/../zone.points.json#zone_temp",
            "/points/zone.points.json#zone_temp",
            "points/zone.points.json",
        ):
            with self.subTest(target=target):
                self.mutate(
                    "points/vav.points.json",
                    lambda value, ref=target: value["aliases"][0].update(target=ref),
                )
                self.assert_invalid(
                    "aliases[0].target: must be points/<family>.points.json#<name>"
                )
                self.restore("points/vav.points.json")

        self.mutate(
            "points/vav.points.json", lambda value: value.update(imports=[])
        )
        self.assert_invalid("target path 'points/zone.points.json' is not in imports")
        self.restore("points/vav.points.json")

        self.mutate(
            "points/vav.points.json",
            lambda value: value["aliases"][0].update(
                target="points/zone.points.json#missing_point"
            ),
        )
        self.assert_invalid("concrete point 'points/zone.points.json#missing_point' is missing")
        self.restore("points/vav.points.json")

        self.mutate(
            "points/zone.points.json",
            lambda value: value.update(
                imports=["points/ahu.points.json"],
                aliases=[
                    {
                        "name": "legacy_zone_temp",
                        "target": "points/ahu.points.json#sat",
                    }
                ],
            ),
        )
        self.mutate(
            "points/vav.points.json",
            lambda value: value["aliases"][0].update(
                target="points/zone.points.json#legacy_zone_temp"
            ),
        )
        self.assert_invalid("alias-to-alias target")


if __name__ == "__main__":
    unittest.main()
