"""모듈 B 회귀 테스트 (표준 라이브러리 unittest). 실행: python -m unittest discover -s tests -v"""

import sys
import tempfile
import unittest
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

import server_inspector as si  # noqa: E402


class JudgeTest(unittest.TestCase):
    def test_threshold_boundaries(self):
        self.assertEqual(si.judge(69.9, 70, 85), si.OK)
        self.assertEqual(si.judge(70.0, 70, 85), si.WARNING)
        self.assertEqual(si.judge(84.9, 70, 85), si.WARNING)
        self.assertEqual(si.judge(85.0, 70, 85), si.CRITICAL)

    def test_service_stopped_is_critical(self):
        r = si.Record(datetime(2026, 1, 1), 10, 10, 10, 10, "Stopped", [])
        self.assertEqual(si.judge_record(r, si.Thresholds())["Core Service"], si.CRITICAL)


class SustainedTest(unittest.TestCase):
    def test_run_of_three_is_sustained(self):
        statuses = [si.OK, si.WARNING, si.WARNING, si.WARNING, si.OK]
        self.assertEqual(si.mark_sustained(statuses), [False, True, True, True, False])

    def test_short_run_is_not_sustained(self):
        statuses = [si.CRITICAL, si.CRITICAL, si.OK, si.WARNING]
        self.assertEqual(si.mark_sustained(statuses), [False] * 4)


class ParseTest(unittest.TestCase):
    def test_percent_with_sign_and_bounds(self):
        self.assertEqual(si.parse_percent(" 88.4% ", "x"), 88.4)
        with self.assertRaises(ValueError):
            si.parse_percent("120", "x")
        with self.assertRaises(ValueError):
            si.parse_percent("abc", "x")

    def test_processes(self):
        self.assertEqual(
            si.parse_processes("sqlservr.exe:12.5;chrome.exe:3"),
            [("sqlservr.exe", 12.5), ("chrome.exe", 3.0)],
        )

    def test_missing_column_raises(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "bad.csv"
            path.write_text("timestamp,cpu_pct\n2026-01-01 00:00:00,10\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                si.load_records(path)


class PipelineTest(unittest.TestCase):
    def test_sample_pipeline(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            sample = tmp_path / "sample.csv"
            self.assertEqual(si.make_sample(sample), 144)

            diagnoses, summaries, grade = si.run(sample, tmp_path / "out", si.Thresholds())
            by_name = {s.name: s for s in summaries}

            self.assertEqual(len(diagnoses), 144)
            self.assertEqual(grade, "CRITICAL")
            self.assertEqual(by_name["Disk D:"].worst, si.CRITICAL)
            self.assertTrue(by_name["Disk D:"].sustained)
            self.assertTrue(by_name["CPU"].sustained)
            self.assertEqual(by_name["Core Service"].crit_count, 2)
            self.assertFalse(by_name["Core Service"].sustained)  # 2회라 지속 부하 아님

            report = tmp_path / "out" / "reports" / "server_inspection_summary.md"
            self.assertTrue(report.exists())
            text = report.read_text(encoding="utf-8")
            self.assertIn("AIRBOOK-FAS-DEMO", text)
            self.assertIn("88.7%", text)
            self.assertTrue((tmp_path / "out" / "reports" / "server_diagnosis.csv").exists())

    def test_hostname_is_masked_in_outputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            src = tmp_path / "in.csv"
            src.write_text(
                "timestamp,hostname,cpu_pct,mem_pct,disk_c_pct,disk_d_pct,core_service,top_processes\n"
                "2026-01-01 00:00:00,REAL-HOST-NAME,10,10,10,10,Running,\n",
                encoding="utf-8",
            )
            si.run(src, tmp_path / "out", si.Thresholds())
            text = (tmp_path / "out" / "reports" / "server_diagnosis.csv").read_text(encoding="utf-8")
            self.assertNotIn("REAL-HOST-NAME", text)
            self.assertIn(si.HOSTNAME_MASK, text)


if __name__ == "__main__":
    unittest.main()
