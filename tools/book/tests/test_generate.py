import tempfile
import unittest
from pathlib import Path

from tools.book import generate
from tools.point_resolution import load_point_corpus


PRODUCT_ROOT = Path(__file__).resolve().parents[3]


class PointBookGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = load_point_corpus(PRODUCT_ROOT).require_valid()

    def render(self, fault_id):
        family = fault_id.split("-")[0].lower()
        fault_dir = PRODUCT_ROOT / "faults" / family / fault_id
        frontmatter, body = generate.read_card(fault_dir / "card.md")
        return generate.render_fault_page(
            fault_id, fault_dir, frontmatter, body, {fault_id}, self.corpus
        )

    def test_fault_pages_link_aliases_to_zone_and_local_points_to_family(self):
        vav = self.render("VAV-0003")
        for name in ("zone_temp", "zone_temp_sp_htg", "zone_temp_sp_clg"):
            self.assertIn(f"[`{name}`](../../points/zone.md#{name})", vav)
        self.assertIn("[`rht_vlv_cmd`](../../points/vav.md#rht_vlv_cmd)", vav)

        system = self.render("SYS-0003")
        self.assertIn("[`occ_sensor`](../../points/zone.md#occ_sensor)", system)
        self.assertIn(
            "[`lighting_status`](../../points/sys.md#lighting_status)", system
        )
        self.assertIn("[`occ_scheduled`](../../points/sys.md#occ_scheduled)", system)

    def test_point_pages_render_alias_links_without_duplicate_anchors(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            destination = Path(temporary_directory) / "points"
            pages = generate.build_points(self.corpus, destination)
            self.assertEqual(len(pages), 15)
            vav = (destination / "vav.md").read_text(encoding="utf-8")
            system = (destination / "sys.md").read_text(encoding="utf-8")
            zone = (destination / "zone.md").read_text(encoding="utf-8")

        self.assertIn("## Compatibility aliases", vav)
        self.assertIn(
            "[`zone_temp`](zone.md#zone_temp) → `points/zone.points.json#zone_temp`",
            vav,
        )
        self.assertNotIn("\n## zone_temp {#zone_temp}\n", vav)
        self.assertIn("\n## zone_airflow {#zone_airflow}\n", vav)
        self.assertIn(
            "[`occ_sensor`](zone.md#occ_sensor) → `points/zone.points.json#occ_sensor`",
            system,
        )
        self.assertNotIn("\n## occ_sensor {#occ_sensor}\n", system)
        for name in ("zone_temp", "zone_temp_sp_htg", "zone_temp_sp_clg", "occ_sensor"):
            self.assertIn(f"\n## {name} {{#{name}}}\n", zone)


if __name__ == "__main__":
    unittest.main()
