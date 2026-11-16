"""AIRBOOK-FAS 서버 점검 결과 진단기 (모듈 B).

- 점검 CSV를 표준 레코드로 정규화하고 임계치로 OK / WARNING / CRITICAL 판정
- 연속 3회 이상 주의·위험이 이어진 지표에 '지속적 부하' 태그 부여
- 위험 항목별 조치 권고와 고부하 프로세스 요약을 Markdown / CSV로 저장
- --make-sample 로 가상 점검 데이터 생성 (실서버 데이터 아님)
- 표준 라이브러리만 사용
"""

from __future__ import annotations

import argparse
import csv
import math
import random
import sys
from collections import Counter
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional

HOSTNAME_MASK = "AIRBOOK-FAS-DEMO"  # 실제 호스트명은 출력에 남기지 않음
SUSTAINED_RUN = 3  # 연속 이 횟수 이상 주의/위험이면 지속적 부하
OK, WARNING, CRITICAL = "OK", "WARNING", "CRITICAL"
STATUS_RANK = {OK: 0, WARNING: 1, CRITICAL: 2}
METRICS = ("CPU", "MEM", "Disk C:", "Disk D:", "Core Service")
REQUIRED_COLUMNS = ("timestamp", "cpu_pct", "mem_pct", "disk_c_pct", "disk_d_pct", "core_service")
CSV_HEADER = ("timestamp", "cpu_pct", "mem_pct", "disk_c_pct", "disk_d_pct", "core_service", "top_processes")


@dataclass(frozen=True)
class Thresholds:
    cpu_warn: float = 70.0
    cpu_crit: float = 85.0
    mem_warn: float = 80.0
    mem_crit: float = 90.0
    disk_warn: float = 75.0
    disk_crit: float = 85.0


@dataclass
class Record:
    timestamp: datetime
    cpu: float
    mem: float
    disk_c: float
    disk_d: float
    core_service: str  # Running / Stopped / Unknown
    top_processes: list[tuple[str, float]]


@dataclass
class Diagnosis:
    record: Record
    status: dict[str, str]
    sustained: dict[str, bool]


@dataclass
class MetricSummary:
    name: str
    worst: str
    peak_value: Optional[float]
    peak_time: Optional[datetime]
    warn_count: int
    crit_count: int
    sustained: bool
    latest: str


# ---------- 파싱 ----------

def parse_percent(text: str, field: str) -> float:
    cleaned = text.strip().rstrip("%").strip()
    try:
        value = float(cleaned)
    except ValueError as exc:
        raise ValueError(f"{field} 값 해석 실패: {text!r}") from exc
    if not 0.0 <= value <= 100.0:
        raise ValueError(f"{field} 값 범위 초과: {value}")
    return value


def parse_timestamp(text: str) -> datetime:
    for fmt in ("%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M"):
        try:
            return datetime.strptime(text.strip(), fmt)
        except ValueError:
            continue
    raise ValueError(f"시각 해석 실패: {text!r}")


def parse_processes(text: str) -> list[tuple[str, float]]:
    """'name:pct;name:pct' 형식을 리스트로 변환한다."""
    result: list[tuple[str, float]] = []
    for item in (part.strip() for part in text.split(";")):
        if not item:
            continue
        name, sep, pct = item.rpartition(":")
        if not sep or not name:
            continue
        result.append((name, parse_percent(pct, "process")))
    return result


def normalize_service(text: str) -> str:
    lowered = text.strip().lower()
    if lowered == "running":
        return "Running"
    if lowered == "stopped":
        return "Stopped"
    return "Unknown"


def load_records(path: Path) -> list[Record]:
    with path.open(encoding="utf-8-sig", newline="") as f:
        reader = csv.DictReader(f)
        missing = [c for c in REQUIRED_COLUMNS if c not in (reader.fieldnames or [])]
        if missing:
            raise ValueError(f"필수 열 누락: {missing}")
        records: list[Record] = []
        for line_no, row in enumerate(reader, start=2):
            try:
                records.append(Record(
                    timestamp=parse_timestamp(row["timestamp"]),
                    cpu=parse_percent(row["cpu_pct"], "cpu_pct"),
                    mem=parse_percent(row["mem_pct"], "mem_pct"),
                    disk_c=parse_percent(row["disk_c_pct"], "disk_c_pct"),
                    disk_d=parse_percent(row["disk_d_pct"], "disk_d_pct"),
                    core_service=normalize_service(row["core_service"]),
                    top_processes=parse_processes(row.get("top_processes") or ""),
                ))
            except ValueError as exc:
                raise ValueError(f"{line_no}행: {exc}") from exc
    if not records:
        raise ValueError("데이터 행이 없습니다.")
    records.sort(key=lambda r: r.timestamp)
    return records


# ---------- 진단 ----------

def judge(value: float, warn: float, crit: float) -> str:
    if value >= crit:
        return CRITICAL
    if value >= warn:
        return WARNING
    return OK


def judge_record(r: Record, th: Thresholds) -> dict[str, str]:
    return {
        "CPU": judge(r.cpu, th.cpu_warn, th.cpu_crit),
        "MEM": judge(r.mem, th.mem_warn, th.mem_crit),
        "Disk C:": judge(r.disk_c, th.disk_warn, th.disk_crit),
        "Disk D:": judge(r.disk_d, th.disk_warn, th.disk_crit),
        "Core Service": {"Running": OK, "Stopped": CRITICAL}.get(r.core_service, WARNING),
    }


def mark_sustained(statuses: list[str]) -> list[bool]:
    """OK이 아닌 상태가 SUSTAINED_RUN 이상 연속된 구간의 모든 시점에 True를 표시한다."""
    flags = [False] * len(statuses)
    i = 0
    while i < len(statuses):
        if statuses[i] == OK:
            i += 1
            continue
        j = i
        while j < len(statuses) and statuses[j] != OK:
            j += 1
        if j - i >= SUSTAINED_RUN:
            for k in range(i, j):
                flags[k] = True
        i = j
    return flags


def diagnose(records: list[Record], th: Thresholds) -> list[Diagnosis]:
    per_record = [judge_record(r, th) for r in records]
    sustained_by_metric = {
        m: mark_sustained([status[m] for status in per_record]) for m in METRICS
    }
    return [
        Diagnosis(
            record=r,
            status=per_record[i],
            sustained={m: sustained_by_metric[m][i] for m in METRICS},
        )
        for i, r in enumerate(records)
    ]


def metric_value(r: Record, metric: str) -> Optional[float]:
    return {
        "CPU": r.cpu,
        "MEM": r.mem,
        "Disk C:": r.disk_c,
        "Disk D:": r.disk_d,
    }.get(metric)


def summarize(diagnoses: list[Diagnosis]) -> list[MetricSummary]:
    summaries: list[MetricSummary] = []
    for m in METRICS:
        worst = OK
        warn = crit = 0
        for d in diagnoses:
            s = d.status[m]
            worst = max(worst, s, key=lambda x: STATUS_RANK[x])
            warn += s == WARNING
            crit += s == CRITICAL
        values = [(metric_value(d.record, m), d.record.timestamp) for d in diagnoses]
        values = [(v, t) for v, t in values if v is not None]
        peak_value, peak_time = max(values, key=lambda x: x[0]) if values else (None, None)
        summaries.append(MetricSummary(
            name=m,
            worst=worst,
            peak_value=peak_value,
            peak_time=peak_time,
            warn_count=warn,
            crit_count=crit,
            sustained=any(d.sustained[m] for d in diagnoses),
            latest=diagnoses[-1].status[m],
        ))
    return summaries


def overall_grade(summaries: list[MetricSummary]) -> str:
    worst = max((s.worst for s in summaries), key=lambda x: STATUS_RANK[x])
    if worst == CRITICAL:
        return "CRITICAL"
    if worst == WARNING or any(s.sustained for s in summaries):
        return "WARNING"
    return "HEALTHY"


def top_process_at_peak(diagnoses: list[Diagnosis], metric: str) -> Optional[tuple[str, float, datetime]]:
    """지정 지표가 가장 높았던 시점의 고점유 프로세스를 반환한다."""
    flagged = [d for d in diagnoses if d.status[metric] != OK and d.record.top_processes]
    if not flagged:
        return None
    target = max(flagged, key=lambda d: metric_value(d.record, metric) or 0.0)
    name, pct = max(target.record.top_processes, key=lambda x: x[1])
    return name, pct, target.record.timestamp


def build_recommendations(summaries: list[MetricSummary], diagnoses: list[Diagnosis]) -> list[str]:
    recs: list[str] = []
    by_name = {s.name: s for s in summaries}

    for name in ("Disk D:", "Disk C:"):
        s = by_name[name]
        if s.worst == OK and not s.sustained:
            continue
        tag = " (지속적 부하)" if s.sustained else ""
        if name == "Disk D:":
            action = "오래된 덤프/로그 아카이빙 권고"
        else:
            action = "임시 파일·업데이트 캐시 정리 및 시스템 드라이브 여유 공간 확보 권고"
        recs.append(f"{name} 사용률 {s.peak_value:.1f}% 도달{tag} — {action}")

    s = by_name["CPU"]
    if s.worst != OK or s.sustained:
        tag = " (지속적 부하)" if s.sustained else ""
        proc = top_process_at_peak(diagnoses, "CPU")
        proc_text = f" 원인 후보: {proc[0]} ({proc[1]:.1f}%)" if proc else ""
        recs.append(f"CPU 최고 {s.peak_value:.1f}%{tag} —{proc_text} 고부하 프로세스 점검 및 배치 작업 시간 분산 권고")

    s = by_name["MEM"]
    if s.worst != OK or s.sustained:
        tag = " (지속적 부하)" if s.sustained else ""
        recs.append(f"메모리 최고 {s.peak_value:.1f}%{tag} — 점유 상위 프로세스 및 메모리 누수 여부 점검 권고")

    s = by_name["Core Service"]
    if s.crit_count:
        recs.append(f"핵심 서비스 중지 {s.crit_count}회 감지 — 서비스 이벤트 로그 확인 및 재기동 절차 점검 권고")

    return recs


# ---------- 출력 ----------

def write_diagnosis_csv(path: Path, diagnoses: list[Diagnosis]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    header = ["timestamp", "hostname"]
    for m in METRICS:
        header += [f"{m} value" if m != "Core Service" else "Core Service value", f"{m} status", f"{m} sustained"]
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(header)
        for d in diagnoses:
            r = d.record
            row: list[object] = [r.timestamp.isoformat(timespec="minutes"), HOSTNAME_MASK]
            for m in METRICS:
                v = metric_value(r, m)
                row += [r.core_service if v is None else v, d.status[m], d.sustained[m]]
            writer.writerow(row)


def write_markdown(path: Path, diagnoses: list[Diagnosis], summaries: list[MetricSummary],
                   th: Thresholds, grade: str, recs: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    first, last = diagnoses[0].record.timestamp, diagnoses[-1].record.timestamp
    alarms = [d for d in diagnoses if any(v != OK for v in d.status.values())]
    thresholds_text = {
        "CPU": f"주의 {th.cpu_warn:g}% / 위험 {th.cpu_crit:g}%",
        "MEM": f"주의 {th.mem_warn:g}% / 위험 {th.mem_crit:g}%",
        "Disk C:": f"주의 {th.disk_warn:g}% / 위험 {th.disk_crit:g}%",
        "Disk D:": f"주의 {th.disk_warn:g}% / 위험 {th.disk_crit:g}%",
        "Core Service": "Stopped = 위험",
    }
    lines = [
        "# AIRBOOK-FAS 서버 점검 진단 요약",
        "",
        f"- 점검 대상: {HOSTNAME_MASK}",
        f"- 점검 기간: {first:%Y-%m-%d %H:%M} ~ {last:%Y-%m-%d %H:%M} ({len(diagnoses)}건)",
        f"- 종합 등급: **{grade}**",
        "",
        "## 지표별 진단",
        "",
        "| 항목 | 임계치 | 최고값 | 발생 시각 | 주의 | 위험 | 지속 부하 | 최종 상태 |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for s in summaries:
        peak = "-" if s.peak_value is None else f"{s.peak_value:.1f}%"
        when = "-" if s.peak_time is None else f"{s.peak_time:%m-%d %H:%M}"
        lines.append(
            f"| {s.name} | {thresholds_text[s.name]} | {peak} | {when} | {s.warn_count} | "
            f"{s.crit_count} | {'Y' if s.sustained else '-'} | {s.latest} |"
        )

    lines += ["", "## 조치 권고", ""]
    lines += [f"- {rec}" for rec in recs] if recs else ["- 특이 사항 없음"]

    lines += ["", "## 경보 로그 (주의 이상 시점, 최대 30건)", ""]
    if alarms:
        lines += ["| 시각 | CPU | MEM | Disk C: | Disk D: | Core | 비고 |", "|---|---|---|---|---|---|---|"]
        for d in alarms[:30]:
            r = d.record
            flags = [
                m + ("(지속)" if d.sustained[m] else "")
                for m in METRICS if d.status[m] != OK
            ]
            lines.append(
                f"| {r.timestamp:%m-%d %H:%M} | {r.cpu:.1f} | {r.mem:.1f} | {r.disk_c:.1f} | "
                f"{r.disk_d:.1f} | {r.core_service} | {', '.join(flags)} |"
            )
    else:
        lines.append("- 경보 없음")

    lines += ["", "## 고부하 프로세스 빈도 (CPU 주의 이상 시점 기준)", ""]
    counter: Counter[str] = Counter()
    for d in diagnoses:
        if d.status["CPU"] != OK:
            counter.update(name for name, _ in d.record.top_processes)
    lines += [f"- {name}: {count}회" for name, count in counter.most_common(5)] or ["- 해당 없음"]
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


# ---------- 가상 데이터 생성 ----------

def make_sample(path: Path, seed: int = 7) -> int:
    rng = random.Random(seed)
    start = datetime(2026, 1, 1, 0, 0)
    procs = ["sqlservr.exe", "w3wp.exe", "AirbookCore.exe", "backup_agent.exe", "chrome.exe", "svchost.exe"]
    path.parent.mkdir(parents=True, exist_ok=True)
    rows: list[list[str]] = []
    for i in range(144):  # 24시간, 10분 간격
        t = start + timedelta(minutes=10 * i)
        hour = t.hour + t.minute / 60
        cpu = 28 + 6 * math.sin(2 * math.pi * (hour - 6) / 24) + rng.gauss(0, 3)
        if 10 <= hour < 11.5:  # 배치 작업 구간: 지속 부하
            cpu = 84 + rng.gauss(0, 3)
        if rng.random() < 0.03:  # 일시적 피크
            cpu += rng.uniform(20, 35)
        cpu = min(max(cpu, 1.0), 99.5)

        mem = 58 + 18 * (hour / 24) + rng.gauss(0, 1.5)
        if 15 <= hour < 15.5:  # 오후 메모리 고점
            mem = 91 + rng.gauss(0, 0.5)
        mem = min(max(mem, 1.0), 99.5)

        disk_c = 55.0 + (i / 143) * 2 + rng.gauss(0, 0.2)
        disk_d = 80.0 + (i / 143) * 8.6 + rng.gauss(0, 0.3)
        service = "Stopped" if i in (20, 21) else "Running"

        weights = sorted((rng.random() for _ in range(5)), reverse=True)
        names = rng.sample(procs, 5)
        if 10 <= hour < 11.5:
            names[0] = "sqlservr.exe"
        top = ";".join(f"{n}:{cpu * w / sum(weights) * 0.9:.1f}" for n, w in zip(names, weights))

        rows.append([
            t.strftime("%Y-%m-%d %H:%M:%S"), f"{cpu:.1f}", f"{mem:.1f}",
            f"{disk_c:.1f}", f"{disk_d:.1f}", service, top,
        ])
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(CSV_HEADER)
        writer.writerows(rows)
    return len(rows)


# ---------- 실행 ----------

def run(input_path: Path, output_dir: Path, th: Thresholds) -> tuple[list[Diagnosis], list[MetricSummary], str]:
    records = load_records(input_path)
    diagnoses = diagnose(records, th)
    summaries = summarize(diagnoses)
    grade = overall_grade(summaries)
    recs = build_recommendations(summaries, diagnoses)
    write_markdown(output_dir / "reports" / "server_inspection_summary.md", diagnoses, summaries, th, grade, recs)
    write_diagnosis_csv(output_dir / "reports" / "server_diagnosis.csv", diagnoses)
    return diagnoses, summaries, grade


def main(argv: Optional[list[str]] = None) -> int:
    p = argparse.ArgumentParser(description="AIRBOOK-FAS 서버 점검 진단기")
    p.add_argument("--input", type=Path, default=Path("sample_data/airbook_fas_metrics.csv"))
    p.add_argument("--output-dir", type=Path, default=Path("outputs"))
    p.add_argument("--make-sample", type=Path, metavar="PATH", help="가상 점검 CSV를 생성하고 종료")
    p.add_argument("--cpu-warn", type=float, default=Thresholds.cpu_warn)
    p.add_argument("--cpu-crit", type=float, default=Thresholds.cpu_crit)
    p.add_argument("--mem-warn", type=float, default=Thresholds.mem_warn)
    p.add_argument("--mem-crit", type=float, default=Thresholds.mem_crit)
    p.add_argument("--disk-warn", type=float, default=Thresholds.disk_warn)
    p.add_argument("--disk-crit", type=float, default=Thresholds.disk_crit)
    args = p.parse_args(argv)

    if args.make_sample:
        n = make_sample(args.make_sample)
        print(f"[sample] 가상 점검 데이터 {n}행 생성 -> {args.make_sample}")
        return 0

    th = Thresholds(args.cpu_warn, args.cpu_crit, args.mem_warn, args.mem_crit,
                    args.disk_warn, args.disk_crit)
    try:
        diagnoses, summaries, grade = run(args.input, args.output_dir, th)
    except (OSError, ValueError) as exc:
        print(f"[오류] {exc}", file=sys.stderr)
        return 1

    print(f"[종합] {grade}  (점검 {len(diagnoses)}건, 호스트 {HOSTNAME_MASK})")
    print(f"{'항목':<14}{'최고값':>10}  {'주의':>4} {'위험':>4}  {'지속':>4}  최종")
    for s in summaries:
        peak = "-" if s.peak_value is None else f"{s.peak_value:.1f}%"
        print(f"{s.name:<14}{peak:>10}  {s.warn_count:>4} {s.crit_count:>4}  "
              f"{'Y' if s.sustained else '-':>4}  {s.latest}")
    print(f"[산출물] {args.output_dir / 'reports'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
