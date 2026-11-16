"""FSC400 초음파 유량계 실시간 관제 GUI (모듈 A).

- 데이터 소스: MODBUS-RTU (매뉴얼 부록 A) / ASCII 라인 (samplecode 방식) / 시뮬레이션
- 수신 데이터는 샘플마다 일별 CSV (data/flow_log_YYYY-MM-DD.csv) 에 기록
- 실시간 차트·수치 카드·목록 표시, 달력으로 과거 기록 조회 및 차트 클릭 연동
"""

import bisect
import calendar
import csv
import json
import math
import queue
import random
import struct
import sys
import threading
import time
from collections import deque
from dataclasses import dataclass
from datetime import date, datetime
from pathlib import Path
from typing import Optional

import matplotlib
import matplotlib.dates as mdates
from matplotlib import font_manager
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg, NavigationToolbar2Tk
from matplotlib.figure import Figure
import tkinter as tk
from tkinter import ttk
import ttkbootstrap as tb
from ttkbootstrap.dialogs import Messagebox

try:
    import serial
except ImportError:
    serial = None

BASE_DIR = Path(sys.executable).parent if getattr(sys, "frozen", False) else Path(__file__).resolve().parents[1]
DATA_DIR = BASE_DIR / "data"
SETTINGS_PATH = BASE_DIR / "settings.json"

CSV_HEADER = ["Timestamp", "Header", "Flow_Rate(m3/h)", "Total_Flow(m3)", "Velocity(m/s)", "Error_Code"]
TIME_FMT = "%Y-%m-%d %H:%M:%S"

# 매뉴얼 부록 A: 순간 유량 0-1, 합산 누적 정수부 4-5, 소수부 16, 순간 유속 24-25
REG_COUNT = 26
SRC_MODBUS = "MODBUS-RTU"
SRC_ASCII = "ASCII 라인"
SRC_SIM = "시뮬레이션"
SOURCES = [SRC_MODBUS, SRC_ASCII, SRC_SIM]
DEFAULT_SETTINGS = {"source": SRC_MODBUS, "port": "COM3", "baud": 9600, "slave": 1, "interval": 1.0}

LIVE_POINTS = 300
LIVE_TABLE_MAX = 200
PAGE_SIZE = 500


@dataclass
class Sample:
    timestamp: datetime
    header: str
    flow: float
    total: float
    velocity: float
    error: str


# ---------- MODBUS-RTU ----------

def crc16_modbus(data: bytes) -> int:
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def build_read_request(slave: int, start: int, count: int) -> bytes:
    body = struct.pack(">BBHH", slave, 0x04, start, count)
    return body + struct.pack("<H", crc16_modbus(body))


def decode_cdab_float(regs: list[int]) -> float:
    raw = struct.pack(">HH", regs[0], regs[1])
    return struct.unpack(">f", bytes([raw[2], raw[3], raw[0], raw[1]]))[0]


def parse_response(frame: bytes, slave: int, count: int) -> list[int]:
    expected = 3 + 2 * count + 2
    if len(frame) != expected:
        raise IOError(f"응답 길이 불일치 ({len(frame)}/{expected})")
    if crc16_modbus(frame[:-2]) != struct.unpack("<H", frame[-2:])[0]:
        raise IOError("CRC 오류")
    if frame[0] != slave:
        raise IOError(f"슬레이브 주소 불일치 ({frame[0]})")
    if frame[1] & 0x80:
        raise IOError(f"MODBUS 예외 응답 (코드 {frame[2]})")
    if frame[1] != 0x04 or frame[2] != 2 * count:
        raise IOError("기능 코드 또는 바이트 수 불일치")
    return [struct.unpack(">H", frame[3 + i:5 + i])[0] for i in range(0, 2 * count, 2)]


def regs_to_sample(regs: list[int]) -> Sample:
    flow = decode_cdab_float(regs[0:2])
    velocity = decode_cdab_float(regs[24:26])
    total = ((regs[5] << 16) | regs[4]) + regs[16] * 0.001
    return Sample(datetime.now(), "FSC400", flow, total, velocity, "N/A")


# ---------- 데이터 소스 ----------

class ModbusSource:
    def __init__(self, port: str, baud: int, slave: int, interval: float) -> None:
        if serial is None:
            raise RuntimeError("pyserial이 설치되어 있지 않습니다")
        self.interval = interval
        self.slave = slave
        self.ser = serial.Serial(port, baudrate=baud, bytesize=8, parity=serial.PARITY_NONE,
                                 stopbits=1, timeout=0.5)

    def read(self) -> Optional[Sample]:
        self.ser.reset_input_buffer()
        self.ser.write(build_read_request(self.slave, 0, REG_COUNT))
        frame = self.ser.read(3 + 2 * REG_COUNT + 2)
        return regs_to_sample(parse_response(frame, self.slave, REG_COUNT))

    def close(self) -> None:
        self.ser.close()


class AsciiSource:
    """'헤더,유량,적산,에러,유속' 형식의 한 줄 데이터 (samplecode 방식). 줄 단위로 도착하므로 주기 대기 없음."""
    interval = 0.0

    def __init__(self, port: str, baud: int) -> None:
        if serial is None:
            raise RuntimeError("pyserial이 설치되어 있지 않습니다")
        self.ser = serial.Serial(port, baudrate=baud, bytesize=8, parity=serial.PARITY_NONE,
                                 stopbits=1, timeout=1.5)

    def read(self) -> Optional[Sample]:
        line = self.ser.readline().decode("ascii", errors="ignore").strip()
        parts = [p.strip() for p in line.split(",")]
        if len(parts) < 4:
            return None
        try:
            return Sample(datetime.now(), parts[0], float(parts[1].replace("+", "")),
                          float(parts[2].replace("+", "")), float(parts[4]) if len(parts) > 4 else 0.0,
                          parts[3])
        except ValueError:
            return None

    def close(self) -> None:
        self.ser.close()


class SimulationSource:
    def __init__(self, interval: float) -> None:
        self.interval = interval
        self.flow = 22.8
        self.total = 125.45

    def read(self) -> Optional[Sample]:
        self.flow = max(4.0, min(48.0, self.flow + random.uniform(-1.5, 1.5)))
        self.total += self.flow / 3600.0
        velocity = self.flow / 12.8 + random.uniform(-0.02, 0.02)
        error = "1100000000" if random.random() > 0.99 else "0000000000"
        return Sample(datetime.now(), "SIM_FSC400", round(self.flow, 3), round(self.total, 3),
                      round(velocity, 2), error)

    def close(self) -> None:
        pass


def make_source(cfg: dict):
    if cfg["source"] == SRC_MODBUS:
        return ModbusSource(cfg["port"], cfg["baud"], cfg["slave"], cfg["interval"])
    if cfg["source"] == SRC_ASCII:
        return AsciiSource(cfg["port"], cfg["baud"])
    return SimulationSource(cfg["interval"])


class SourceWorker(threading.Thread):
    """수신 스레드: 샘플을 큐에 넣고, 끊기면 재연결을 반복한다. GUI는 큐만 읽는다."""
    MAX_FAILS = 5

    def __init__(self, cfg: dict, out_q: queue.Queue, status_q: queue.Queue, stop_evt: threading.Event):
        super().__init__(daemon=True)
        self.cfg = cfg
        self.out_q = out_q
        self.status_q = status_q
        self.stop_evt = stop_evt

    def _status(self, text: str, style: str) -> None:
        self.status_q.put((text, style))

    def _wait(self, seconds: float) -> None:
        self.stop_evt.wait(max(0.0, seconds))

    def run(self) -> None:
        while not self.stop_evt.is_set():
            try:
                src = make_source(self.cfg)
            except Exception as exc:
                self._status(f"연결 실패: {exc}", "danger")
                self._wait(3)
                continue
            self._status(f"수신 중 ({self.cfg['source']})", "success")
            fails = 0
            try:
                while not self.stop_evt.is_set():
                    t0 = time.monotonic()
                    try:
                        sample = src.read()
                        fails = 0
                    except (OSError, ValueError) as exc:
                        sample = None
                        fails += 1
                        self._status(f"수신 오류 ({fails}/{self.MAX_FAILS}): {exc}", "warning")
                        if fails >= self.MAX_FAILS:
                            break
                    if sample is not None:
                        self.out_q.put(sample)
                    if src.interval > 0:
                        self._wait(src.interval - (time.monotonic() - t0))
            except Exception as exc:
                self._status(f"수신 중단: {exc}", "danger")
            finally:
                try:
                    src.close()
                except Exception:
                    pass
            if not self.stop_evt.is_set():
                self._status("연결 끊김, 재연결 대기 중", "danger")
                self._wait(3)


# ---------- 설정 / 이력 ----------

def load_settings() -> dict:
    try:
        data = json.loads(SETTINGS_PATH.read_text(encoding="utf-8"))
        return {**DEFAULT_SETTINGS, **data}
    except Exception:
        return dict(DEFAULT_SETTINGS)


def save_settings(cfg: dict) -> None:
    try:
        SETTINGS_PATH.write_text(json.dumps(cfg, ensure_ascii=False, indent=4), encoding="utf-8")
    except OSError:
        pass


def log_path_for(day: date) -> Path:
    return DATA_DIR / f"flow_log_{day:%Y-%m-%d}.csv"


def read_day_log(path: Path) -> list[Sample]:
    samples: list[Sample] = []
    with path.open(encoding="utf-8", newline="") as f:
        reader = csv.reader(f)
        next(reader, None)
        for row in reader:
            if len(row) < 6:
                continue
            try:
                samples.append(Sample(datetime.strptime(row[0], TIME_FMT), row[1], float(row[2]),
                                      float(row[3]), float(row[4]), row[5]))
            except ValueError:
                continue
    samples.sort(key=lambda s: s.timestamp)
    return samples


def setup_chart_font() -> None:
    matplotlib.rcParams["axes.unicode_minus"] = False
    font_file = Path("C:/Windows/Fonts/malgun.ttf")
    if sys.platform == "win32" and font_file.exists():
        font_manager.fontManager.addfont(str(font_file))
        matplotlib.rcParams["font.family"] = font_manager.FontProperties(fname=str(font_file)).get_name()


# ---------- GUI ----------

class FlowDashboard:
    def __init__(self, root: tb.Window) -> None:
        self.root = root
        root.title("FSC400 초음파 유량계 실시간 관제")
        root.geometry("1440x900")
        setup_chart_font()

        self.cfg = load_settings()
        self.out_q: queue.Queue = queue.Queue()
        self.status_q: queue.Queue = queue.Queue()
        self.stop_evt: Optional[threading.Event] = None
        self.worker: Optional[SourceWorker] = None

        self.live_points: deque = deque(maxlen=LIVE_POINTS)
        self.live_samples: deque = deque(maxlen=LIVE_TABLE_MAX)
        self.mode = "live"
        self.history: list[Sample] = []
        self.history_times: list[datetime] = []
        self.history_date: Optional[date] = None
        self.page = 0
        self.chart_dirty = True
        self.last_draw = 0.0

        self.csv_date: Optional[date] = None
        self.csv_fh = None
        self.csv_writer = None

        today = date.today()
        self.cal_year, self.cal_month = today.year, today.month

        self._build_ui()
        self._redraw_calendar()
        self._draw_live()
        root.protocol("WM_DELETE_WINDOW", self.on_close)
        self.root.after(200, self._poll)

    # ----- UI 구성 -----
    def _build_ui(self) -> None:
        main = tb.Frame(self.root, padding=12)
        main.pack(fill=tk.BOTH, expand=True)

        top = tb.Frame(main)
        top.pack(fill=tk.X, pady=(0, 6))
        tb.Label(top, text="📊 FSC400 초음파 유량계 통합 관제", font=("Helvetica", 18, "bold")).pack(side=tk.LEFT)
        self.status_lbl = tb.Label(top, text="중지됨", bootstyle="secondary", font=("Helvetica", 11))
        self.status_lbl.pack(side=tk.RIGHT)

        conn = tb.Labelframe(main, text=" 연결 설정 ", padding=6, bootstyle="info")
        conn.pack(fill=tk.X, pady=(0, 8))
        tb.Label(conn, text="소스").pack(side=tk.LEFT, padx=(4, 2))
        self.src_var = tk.StringVar(value=self.cfg["source"])
        tb.Combobox(conn, values=SOURCES, textvariable=self.src_var, state="readonly", width=12).pack(side=tk.LEFT)
        tb.Label(conn, text="포트").pack(side=tk.LEFT, padx=(10, 2))
        self.port_var = tk.StringVar(value=self.cfg["port"])
        tb.Entry(conn, textvariable=self.port_var, width=10).pack(side=tk.LEFT)
        tb.Label(conn, text="보드레이트").pack(side=tk.LEFT, padx=(10, 2))
        self.baud_var = tk.StringVar(value=str(self.cfg["baud"]))
        tb.Combobox(conn, values=[9600, 19200, 38400, 57600, 115200], textvariable=self.baud_var,
                    width=8).pack(side=tk.LEFT)
        tb.Label(conn, text="슬레이브").pack(side=tk.LEFT, padx=(10, 2))
        self.slave_var = tk.StringVar(value=str(self.cfg["slave"]))
        tb.Spinbox(conn, from_=1, to=247, textvariable=self.slave_var, width=5).pack(side=tk.LEFT)
        tb.Label(conn, text="주기(초)").pack(side=tk.LEFT, padx=(10, 2))
        self.interval_var = tk.StringVar(value=str(self.cfg["interval"]))
        tb.Entry(conn, textvariable=self.interval_var, width=6).pack(side=tk.LEFT)
        self.start_btn = tb.Button(conn, text="▶ 시작", bootstyle="success", width=10, command=self.toggle_run)
        self.start_btn.pack(side=tk.RIGHT, padx=4)

        body = tb.Frame(main)
        body.pack(fill=tk.BOTH, expand=True)

        side = tb.Frame(body, width=440)
        side.pack(side=tk.LEFT, fill=tk.Y, padx=(0, 12))
        side.pack_propagate(False)
        self.flow_lbl = self._card(side, "순시 유량 (m³/h)", "info")
        self.total_lbl = self._card(side, "적산 유량 (m³)", "success")
        self.vel_lbl = self._card(side, "유속 (m/s)", "primary")
        self.err_lbl = self._card(side, "에러 코드", "secondary")
        self.time_lbl = tb.Label(side, text="마지막 수신: -", bootstyle="secondary")
        self.time_lbl.pack(anchor=tk.W, pady=(2, 6))
        self.cal_frame = tb.Labelframe(side, text=" 과거 기록 달력 ", padding=5, bootstyle="primary")
        self.cal_frame.pack(fill=tk.BOTH, expand=True)

        right = tb.Frame(body)
        right.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        chart_box = tb.Labelframe(right, text=" 유량 차트 ", padding=4, bootstyle="secondary")
        chart_box.pack(fill=tk.BOTH, expand=True, pady=(0, 8))
        self.fig = Figure(figsize=(7, 2.8), dpi=100, facecolor="#2b3e50")
        self.ax = self.fig.add_subplot(111)
        self.canvas = FigureCanvasTkAgg(self.fig, master=chart_box)
        self.canvas.get_tk_widget().pack(fill=tk.BOTH, expand=True)
        toolbar_frame = tb.Frame(chart_box)
        toolbar_frame.pack(fill=tk.X)
        self.toolbar = NavigationToolbar2Tk(self.canvas, toolbar_frame)
        self.toolbar.update()
        self.canvas.mpl_connect("button_press_event", self.on_chart_click)

        table_box = tb.Labelframe(right, text=" 데이터 목록 ", padding=4, bootstyle="secondary")
        table_box.pack(fill=tk.X, side=tk.BOTTOM)
        bar = tb.Frame(table_box)
        bar.pack(fill=tk.X, pady=(0, 4))
        self.mode_lbl = tb.Label(bar, text="실시간 모드", bootstyle="success", font=("Helvetica", 10, "bold"))
        self.mode_lbl.pack(side=tk.LEFT)
        self.live_btn = tb.Button(bar, text="실시간으로 돌아가기", bootstyle="outline-success",
                                  command=self.back_to_live)
        self.live_btn.pack(side=tk.RIGHT)
        self.next_btn = tb.Button(bar, text="다음 ▶", bootstyle="outline-secondary", command=self.next_page)
        self.next_btn.pack(side=tk.RIGHT, padx=4)
        self.page_lbl = tb.Label(bar, text="")
        self.page_lbl.pack(side=tk.RIGHT, padx=4)
        self.prev_btn = tb.Button(bar, text="◀ 이전", bootstyle="outline-secondary", command=self.prev_page)
        self.prev_btn.pack(side=tk.RIGHT)

        tree_wrap = tb.Frame(table_box)
        tree_wrap.pack(fill=tk.BOTH, expand=True)
        vsb = tb.Scrollbar(tree_wrap, orient=tk.VERTICAL)
        vsb.pack(side=tk.RIGHT, fill=tk.Y)
        hsb = tb.Scrollbar(tree_wrap, orient=tk.HORIZONTAL)
        hsb.pack(side=tk.BOTTOM, fill=tk.X)
        columns = ("timestamp", "header", "flow", "total", "velocity", "error")
        self.tree = ttk.Treeview(tree_wrap, columns=columns, show="headings",
                                 yscrollcommand=vsb.set, xscrollcommand=hsb.set, height=11)
        vsb.config(command=self.tree.yview)
        hsb.config(command=self.tree.xview)
        headings = {"timestamp": "시간", "header": "헤더", "flow": "순시 유량(m³/h)",
                    "total": "적산 유량(m³)", "velocity": "유속(m/s)", "error": "에러 코드"}
        widths = {"timestamp": 170, "header": 110, "flow": 130, "total": 140, "velocity": 110, "error": 120}
        for col in columns:
            self.tree.heading(col, text=headings[col])
            self.tree.column(col, width=widths[col], anchor=tk.CENTER)
        self.tree.pack(fill=tk.X)

    def _card(self, parent: tk.Widget, title: str, style: str) -> tb.Label:
        frame = tb.Labelframe(parent, text=f" {title} ", padding=6, bootstyle=style)
        frame.pack(fill=tk.X, pady=4)
        lbl = tb.Label(frame, text="-", font=("Courier", 20, "bold"), bootstyle=style)
        lbl.pack(anchor=tk.CENTER, pady=4)
        return lbl

    # ----- 연결 제어 -----
    def toggle_run(self) -> None:
        if self.worker and self.worker.is_alive():
            self.stop()
        else:
            self.start()

    def start(self) -> None:
        try:
            cfg = {
                "source": self.src_var.get(),
                "port": self.port_var.get().strip(),
                "baud": int(self.baud_var.get()),
                "slave": int(self.slave_var.get()),
                "interval": float(self.interval_var.get()),
            }
            if cfg["interval"] <= 0 or not 1 <= cfg["slave"] <= 247:
                raise ValueError("주기는 0보다 커야 하고, 슬레이브 주소는 1~247 범위여야 합니다")
        except ValueError as exc:
            Messagebox.show_error(f"설정값을 확인하세요: {exc}", "입력 오류")
            return
        self.cfg = cfg
        save_settings(cfg)
        self.stop_evt = threading.Event()
        self.worker = SourceWorker(cfg, self.out_q, self.status_q, self.stop_evt)
        self.worker.start()
        self.start_btn.config(text="■ 중지", bootstyle="danger")
        self.status_lbl.config(text="연결 중...", bootstyle="warning")

    def stop(self) -> None:
        if self.stop_evt:
            self.stop_evt.set()
        if self.worker:
            self.worker.join(timeout=3)
        self.worker = None
        self.start_btn.config(text="▶ 시작", bootstyle="success")
        self.status_lbl.config(text="중지됨", bootstyle="secondary")

    # ----- 주기 처리 (GUI 스레드) -----
    def _poll(self) -> None:
        try:
            while True:
                text, style = self.status_q.get_nowait()
                self.status_lbl.config(text=text, bootstyle=style)
        except queue.Empty:
            pass
        try:
            while True:
                self._on_sample(self.out_q.get_nowait())
        except queue.Empty:
            pass
        if self.mode == "live" and self.chart_dirty and time.monotonic() - self.last_draw > 0.5:
            self._draw_live()
        self.root.after(200, self._poll)

    def _on_sample(self, s: Sample) -> None:
        self.flow_lbl.config(text=f"{s.flow:,.3f}")
        self.total_lbl.config(text=f"{s.total:,.3f}")
        self.vel_lbl.config(text=f"{s.velocity:.2f}")
        self.err_lbl.config(text=s.error)
        self.time_lbl.config(text=f"마지막 수신: {s.timestamp:%Y-%m-%d %H:%M:%S}")

        self.live_points.append((s.timestamp, s.flow))
        self.live_samples.append(s)
        self.chart_dirty = True
        if self.mode == "live":
            self._insert_row(s, at_top=True)
            children = self.tree.get_children()
            if len(children) > LIVE_TABLE_MAX:
                self.tree.delete(children[-1])
        self._write_csv(s)

    def _write_csv(self, s: Sample) -> None:
        day = s.timestamp.date()
        try:
            if self.csv_date != day:
                self._close_csv()
                DATA_DIR.mkdir(parents=True, exist_ok=True)
                path = log_path_for(day)
                is_new = not path.exists()
                self.csv_fh = path.open("a", newline="", encoding="utf-8")
                self.csv_writer = csv.writer(self.csv_fh)
                if is_new:
                    self.csv_writer.writerow(CSV_HEADER)
                self.csv_date = day
                if is_new:
                    self._redraw_calendar()
            self.csv_writer.writerow([s.timestamp.strftime(TIME_FMT), s.header, f"{s.flow:.3f}",
                                      f"{s.total:.3f}", f"{s.velocity:.2f}", s.error])
            self.csv_fh.flush()
        except OSError as exc:
            self.status_lbl.config(text=f"CSV 기록 실패: {exc}", bootstyle="danger")

    def _close_csv(self) -> None:
        if self.csv_fh:
            self.csv_fh.close()
        self.csv_fh = None
        self.csv_writer = None
        self.csv_date = None

    # ----- 표/차트 -----
    @staticmethod
    def _row_values(s: Sample) -> tuple:
        return (s.timestamp.strftime(TIME_FMT), s.header, f"{s.flow:.3f}", f"{s.total:.3f}",
                f"{s.velocity:.2f}", s.error)

    def _insert_row(self, s: Sample, at_top: bool) -> None:
        self.tree.insert("", 0 if at_top else tk.END, values=self._row_values(s))

    def _draw_live(self) -> None:
        self.last_draw = time.monotonic()
        self.chart_dirty = False
        self.ax.clear()
        self._style_axes()
        if self.live_points:
            xs = [p[0] for p in self.live_points]
            ys = [p[1] for p in self.live_points]
            self.ax.plot(xs, ys, color="#00bc8c", linewidth=2, label="순시 유량 (실시간)")
            lo, hi = min(ys), max(ys)
            margin = max(1.0, (hi - lo) * 0.15)
            self.ax.set_ylim(max(0.0, lo - margin), hi + margin)
            self.ax.legend(loc="upper left")
        self.ax.set_title(f"실시간 순시 유량 (최근 {len(self.live_points)}개)", color="#ecf0f1")
        self.ax.xaxis.set_major_formatter(mdates.DateFormatter("%H:%M:%S"))
        self.fig.autofmt_xdate()
        self.canvas.draw_idle()

    def _draw_history(self) -> None:
        self.ax.clear()
        self._style_axes()
        if self.history:
            ys = [s.flow for s in self.history]
            self.ax.plot(self.history_times, ys, color="#3498db", linewidth=0.8, label="순시 유량 (이력)")
            self.ax.legend(loc="upper right")
        self.ax.set_title(f"{self.history_date:%Y-%m-%d} 유량 이력 (차트 클릭 시 목록 연동)", color="#ecf0f1")
        self.ax.xaxis.set_major_formatter(mdates.DateFormatter("%H:%M"))
        self.fig.autofmt_xdate()
        self.canvas.draw_idle()

    def _style_axes(self) -> None:
        self.ax.set_facecolor("#1a252f")
        self.ax.tick_params(colors="#bdc3c7")
        self.ax.grid(True, color="#34495e", linestyle="--")

    def on_chart_click(self, event) -> None:
        if self.mode != "history" or event.xdata is None or not self.history_times:
            return
        click = mdates.num2date(event.xdata).replace(tzinfo=None)
        idx = bisect.bisect_left(self.history_times, click)
        idx = min(max(idx, 0), len(self.history_times) - 1)
        if idx > 0 and abs(self.history_times[idx - 1] - click) < abs(self.history_times[idx] - click):
            idx -= 1
        target_page, offset = divmod(idx, PAGE_SIZE)
        if target_page != self.page:
            self.page = target_page
            self._render_page()
        children = self.tree.get_children()
        if offset < len(children):
            self.tree.selection_set(children[offset])
            self.tree.see(children[offset])

    # ----- 과거 기록 -----
    def show_history(self, day: date) -> None:
        path = log_path_for(day)
        if not path.exists():
            Messagebox.show_warning(f"{day:%Y-%m-%d} 기록 파일이 없습니다.", "데이터 없음")
            return
        try:
            samples = read_day_log(path)
        except OSError as exc:
            Messagebox.show_error(f"기록 파일을 읽을 수 없습니다: {exc}", "파일 오류")
            return
        if not samples:
            Messagebox.show_info("선택한 날짜에 기록된 데이터가 없습니다.", "데이터 없음")
            return
        self.mode = "history"
        self.history = samples
        self.history_times = [s.timestamp for s in samples]
        self.history_date = day
        self.page = 0
        self.mode_lbl.config(text=f"과거 기록 모드: {day:%Y-%m-%d} ({len(samples):,}건)", bootstyle="info")
        self.live_btn.config(state=tk.NORMAL)
        self._render_page()
        self._draw_history()

    def back_to_live(self) -> None:
        self.mode = "live"
        self.history = []
        self.history_times = []
        self.history_date = None
        self.page = 0
        self.mode_lbl.config(text="실시간 모드", bootstyle="success")
        self.tree.delete(*self.tree.get_children())
        for s in reversed(self.live_samples):
            self._insert_row(s, at_top=False)
        self._draw_live()

    def _render_page(self) -> None:
        self.tree.delete(*self.tree.get_children())
        start = self.page * PAGE_SIZE
        for s in self.history[start:start + PAGE_SIZE]:
            self._insert_row(s, at_top=False)
        total_pages = max(1, math.ceil(len(self.history) / PAGE_SIZE))
        self.page_lbl.config(text=f"{self.page + 1} / {total_pages} 페이지")
        self.prev_btn.config(state=tk.NORMAL if self.page > 0 else tk.DISABLED)
        self.next_btn.config(state=tk.NORMAL if self.page + 1 < total_pages else tk.DISABLED)

    def prev_page(self) -> None:
        if self.mode == "history" and self.page > 0:
            self.page -= 1
            self._render_page()

    def next_page(self) -> None:
        if self.mode != "history":
            return
        total_pages = math.ceil(len(self.history) / PAGE_SIZE)
        if self.page + 1 < total_pages:
            self.page += 1
            self._render_page()

    # ----- 달력 -----
    def _redraw_calendar(self) -> None:
        for w in self.cal_frame.winfo_children():
            w.destroy()
        head = tb.Frame(self.cal_frame)
        head.pack(fill=tk.X, pady=2)
        tb.Button(head, text="◀", bootstyle="link", width=2, command=lambda: self._shift_month(-1)).pack(side=tk.LEFT)
        tb.Label(head, text=f"{self.cal_year}년 {self.cal_month:02d}월", font=("Helvetica", 11, "bold"),
                 anchor=tk.CENTER).pack(side=tk.LEFT, expand=True, fill=tk.X)
        tb.Button(head, text="▶", bootstyle="link", width=2, command=lambda: self._shift_month(1)).pack(side=tk.RIGHT)

        names = tb.Frame(self.cal_frame)
        names.pack(fill=tk.X)
        for name in ["일", "월", "화", "수", "목", "금", "토"]:
            tb.Label(names, text=name, width=4, anchor=tk.CENTER).pack(side=tk.LEFT, expand=True, fill=tk.X)

        grid = tb.Frame(self.cal_frame)
        grid.pack(fill=tk.BOTH, expand=True)
        first_weekday, days = calendar.monthrange(self.cal_year, self.cal_month)
        start_col = (first_weekday + 1) % 7
        today = date.today()
        for i in range(start_col):
            tb.Label(grid, text="", width=4).grid(row=0, column=i)
        for day in range(1, days + 1):
            d = date(self.cal_year, self.cal_month, day)
            pos = start_col + day - 1
            r, c = divmod(pos, 7)
            exists = log_path_for(d).exists()
            if d > today or (d == today and not exists):
                tb.Button(grid, text=str(day), width=3, bootstyle="dark-outline",
                          state=tk.DISABLED).grid(row=r, column=c, padx=2, pady=2)
            else:
                style = "info" if exists else "secondary"
                tb.Button(grid, text=str(day), width=3, bootstyle=style,
                          command=lambda x=d: self.show_history(x)).grid(row=r, column=c, padx=2, pady=2)

    def _shift_month(self, delta: int) -> None:
        month = self.cal_month + delta
        self.cal_year += (month - 1) // 12
        self.cal_month = (month - 1) % 12 + 1
        self._redraw_calendar()

    # ----- 종료 -----
    def on_close(self) -> None:
        self.stop()
        self._close_csv()
        self.root.destroy()


def main() -> None:
    root = tb.Window(themename="superhero")
    FlowDashboard(root)
    root.mainloop()


if __name__ == "__main__":
    main()
