# -*- coding: utf-8 -*-
"""
Studio Airbook - FAS & EAI Server Health Monitor & Inspection System (v3.0)
Target OS   : Windows Server 2025 / Windows 11 (25H2 Pure Native)
Config File : userset.json (Two-File Architecture: checkserver.exe + userset.json)
Copyright   : (c) 2026 Studio Airbook. All rights reserved.
"""

import sys
import os
import json
import time
import datetime
import threading
import subprocess
import csv
import psutil

try:
    import winsound
except ImportError:
    winsound = None

from PyQt6.QtCore import Qt, QTimer, QRectF, QThread, pyqtSignal, QDate
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLabel, QPushButton, QTabWidget, QTextEdit, QTableWidget,
    QTableWidgetItem, QHeaderView, QLineEdit, QSizePolicy,
    QSpinBox, QCheckBox, QGroupBox, QFormLayout, QMessageBox,
    QScrollArea, QCalendarWidget, QComboBox, QSplitter, QDialog,
    QGridLayout
)
from PyQt6.QtGui import QPainter, QColor, QPen, QFont, QBrush, QCursor


# =====================================================================
# 0. 내장 표준 배치 스크립트 원본 템플릿
# =====================================================================
BUILTIN_BAT_SCRIPTS = {
    "F100-2-001.bat": r"""@echo off
setlocal EnableExtensions EnableDelayedExpansion
for /f "usebackq tokens=1,2" %%A in (`powershell -NoProfile -Command "Get-Date -Format 'yyyyMMdd HHmmss'"`) do (
    set "DATE_PART=%%A"
    set "TIME_PART=%%B"
)
set "TIMESTAMP=%DATE_PART%_%TIME_PART%"
set "BASE_DIR=C:\checkPMI"
set "LOGS_ROOT=%BASE_DIR%\logs"
set "TARGET_DIR=%LOGS_ROOT%\%TIMESTAMP%"
if not exist "%BASE_DIR%" md "%BASE_DIR%"
if not exist "%LOGS_ROOT%" md "%LOGS_ROOT%"
if not exist "%TARGET_DIR%" md "%TARGET_DIR%"
set "TXT_FILE=%TARGET_DIR%\check_result.txt"
set "CSV_FILE=%TARGET_DIR%\check_result.csv"
set /a TOTAL_CNT=0
set /a OK_CNT=0
set /a WARN_CNT=0
set /a ERR_CNT=0
echo Code,Cycle,Device,Category,Target,Seq,Item,Status,Remarks > "%CSV_FILE%"
echo ============================================================================== > "%TXT_FILE%"
echo   FAS Server Inspection Report [F100-2-001] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %TIMESTAMP% >> "%TXT_FILE%"
echo   Target Device   : FAS Server (FAS#1 / FAS#2) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=1"
set "V_ITM=Remote IP 172.21.126.31 Ping Test"
ping -n 1 -w 1000 172.21.126.31 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IP 172.21.126.31 Connection OK"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=IP 172.21.126.31 No Response"
    set /a ERR_CNT+=1
)
call :RECORD_DATA
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=2"
set "V_ITM=TCP/IP NetBIOS Helper Service State"
sc query "lmhosts" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=NetBIOS Helper Service Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=NetBIOS Helper Service Stopped"
    set /a WARN_CNT+=1
)
call :RECORD_DATA
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=3"
set "V_ITM=MSSQLSERVER Service State"
sc query "MSSQLSERVER" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=MSSQLSERVER Service Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=MSSQLSERVER Service Inactive"
    set /a ERR_CNT+=1
)
call :RECORD_DATA
set /a PASS_RATE=(OK_CNT * 100) / TOTAL_CNT
echo. >> "%TXT_FILE%"
echo   - Pass Rate : %PASS_RATE% %% >> "%TXT_FILE%"
echo Inspection Completed! Result saved in: %TARGET_DIR%
exit /b 0
:RECORD_DATA
echo [!V_RES!] !V_CAT! ^> !V_TGT! : !V_ITM!
echo [!V_RES!] !V_CAT! - !V_TGT! / !V_ITM! : !V_REM! >> "%TXT_FILE%"
echo F100-2-001,Daily,FAS Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0
""",
    "F100-1-001-DETAIL.bat": r"""@echo off
setlocal EnableExtensions EnableDelayedExpansion
for /f "usebackq tokens=1,2" %%A in (`powershell -NoProfile -Command "Get-Date -Format 'yyyyMMdd HHmmss'"`) do (
    set "DATE_PART=%%A"
    set "TIME_PART=%%B"
)
set "TIMESTAMP=%DATE_PART%_%TIME_PART%"
set "BASE_DIR=C:\checkPMI"
set "LOGS_ROOT=%BASE_DIR%\logs"
set "TARGET_DIR=%LOGS_ROOT%\%TIMESTAMP%"
if not exist "%BASE_DIR%" md "%BASE_DIR%"
if not exist "%LOGS_ROOT%" md "%LOGS_ROOT%"
if not exist "%TARGET_DIR%" md "%TARGET_DIR%"
set "TXT_FILE=%TARGET_DIR%\check_result.txt"
set "CSV_FILE=%TARGET_DIR%\check_result.csv"
set /a TOTAL_CNT=0
set /a OK_CNT=0
set /a WARN_CNT=0
set /a ERR_CNT=0
echo Code,Cycle,Device,Category,Target,Seq,Item,Status,Remarks > "%CSV_FILE%"
echo ============================================================================== > "%TXT_FILE%"
echo   EAI Server DETAILED Inspection Report [F100-1-001-DETAIL] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#1 EAI Server"
set "V_SEQ=1"
set "V_ITM=EverRun Web Management Port (443)"
netstat -ano | findstr ":443" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 443 Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 443 Inactive"
    set /a WARN_CNT+=1
)
call :RECORD_DATA
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=2"
set "V_ITM=CLAVO Web Port 80 Check"
netstat -ano | findstr ":80" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 80 Open"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 80 Closed"
    set /a WARN_CNT+=1
)
call :RECORD_DATA
set /a PASS_RATE=(OK_CNT * 100) / TOTAL_CNT
echo   - Pass Rate : %PASS_RATE% %% >> "%TXT_FILE%"
echo Detailed EAI Inspection Completed!
exit /b 0
:RECORD_DATA
echo [!V_RES!] !V_CAT! ^> !V_TGT! : !V_ITM!
echo [!V_RES!] !V_CAT! - !V_TGT! / !V_ITM! : !V_REM! >> "%TXT_FILE%"
echo F100-1-001,Daily,EAI Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0
"""
}


# =====================================================================
# 1. 설정 관리자 (userset.json)
# =====================================================================
class ConfigManager:
    BASE_PMI_DIR = r"C:\checkPMI"

    DEFAULT_CONFIG = {
        "system": {
            "interval_sec": 2,
            "sound_alert_enabled": True,
            "beep_freq_hz": 1200,
            "beep_duration_ms": 350,
            "time_sync_cmd": "w32tm /resync /force"
        },
        "thresholds": {
            "cpu_percent": 85,
            "ram_percent": 85,
            "disk_percent": 90,
            "disk_drive": "C:\\",
            "proc_count_limit": 300
        },
        "target_processes": [
            "Gms.Application.exe",
            "FasServer.exe",
            "SMC.exe",
            "bgw.exe",
            "sqlservr.exe"
        ],
        "bat_paths": {
            "daily_fas": r"C:\checkPMI\F100-2-001.bat",
            "daily_eai": r"C:\checkPMI\F100-1-001-DETAIL.bat",
            "monthly": r"C:\checkPMI\F100-3-030.bat",
            "annual": r"C:\checkPMI\F100-4-360.bat"
        }
    }

    @classmethod
    def ensure_default_bats(cls):
        if not os.path.exists(cls.BASE_PMI_DIR):
            try:
                os.makedirs(cls.BASE_PMI_DIR, exist_ok=True)
            except Exception:
                pass

        for fname, content in BUILTIN_BAT_SCRIPTS.items():
            full_path = os.path.join(cls.BASE_PMI_DIR, fname)
            if not os.path.exists(full_path):
                try:
                    with open(full_path, "w", encoding="cp949", errors="replace") as f:
                        f.write(content.strip() + "\n")
                except Exception:
                    pass

    @staticmethod
    def get_config_path():
        if getattr(sys, 'frozen', False):
            base_dir = os.path.dirname(sys.executable)
        else:
            base_dir = os.path.dirname(os.path.abspath(__file__))
        return os.path.join(base_dir, "userset.json")

    @classmethod
    def load(cls):
        cls.ensure_default_bats()
        path = cls.get_config_path()
        if not os.path.exists(path):
            cls.save(cls.DEFAULT_CONFIG)
            return cls.DEFAULT_CONFIG.copy()
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
                # 누락된 키 보정
                if "proc_count_limit" not in data.get("thresholds", {}):
                    data.setdefault("thresholds", {})["proc_count_limit"] = 300
                if "time_sync_cmd" not in data.get("system", {}):
                    data.setdefault("system", {})["time_sync_cmd"] = "w32tm /resync /force"
                return data
        except Exception:
            return cls.DEFAULT_CONFIG.copy()

    @classmethod
    def save(cls, data):
        path = cls.get_config_path()
        try:
            with open(path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=4, ensure_ascii=False)
            return True
        except Exception:
            return False


# =====================================================================
# 2. 배치 파일 비동기 실행 스레드
# =====================================================================
class BatRunnerThread(QThread):
    output_signal = pyqtSignal(str)
    finished_signal = pyqtSignal(bool, str)

    def __init__(self, bat_path, target_dir):
        super().__init__()
        self.bat_path = bat_path
        self.target_dir = target_dir

    def run(self):
        if not os.path.exists(self.bat_path):
            self.output_signal.emit(f"[오류] 배치 파일이 존재하지 않습니다: {self.bat_path}")
            self.finished_signal.emit(False, "")
            return

        self.output_signal.emit(f"[*] 점검 시작: {self.bat_path}")
        try:
            proc = subprocess.Popen(
                ["cmd.exe", "/c", self.bat_path],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                cwd=os.path.dirname(self.bat_path),
                creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
            )

            while True:
                line = proc.stdout.readline()
                if not line:
                    break
                try:
                    decoded = line.decode('cp949', errors='replace').strip()
                except Exception:
                    decoded = line.decode('utf-8', errors='replace').strip()
                if decoded:
                    self.output_signal.emit(decoded)

            proc.wait()
            self.finished_signal.emit(True, self.target_dir)
        except Exception as e:
            self.output_signal.emit(f"[실행 오류 방어] {str(e)}")
            self.finished_signal.emit(False, "")


# =====================================================================
# 3. 도넛 게이지 위젯 (클릭 이벤트 지원)
# =====================================================================
class DonutGaugeWidget(QWidget):
    clicked = pyqtSignal()

    def __init__(self, title="Metric", unit="%", parent=None):
        super().__init__(parent)
        self.title = title
        self.unit = unit
        self.value = 0.0
        self.display_text = "0"
        self.warn_threshold = 85.0
        self.is_alert = False
        self.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        self.setMinimumSize(70, 70)

    def set_data(self, value, is_alert=False, threshold=85.0, custom_text=None, arc_ratio=None):
        self.value = float(value)
        self.is_alert = is_alert
        self.warn_threshold = threshold
        self.display_text = custom_text if custom_text is not None else f"{int(self.value)}{self.unit}"
        self.arc_ratio = arc_ratio if arc_ratio is not None else (self.value / 100.0)
        self.update()

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()
        super().mousePressEvent(event)

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        w, h = self.width(), self.height()
        side = min(w, h)
        cx, cy = w / 2.0, h / 2.0

        margin = max(6.0, side * 0.08)
        radius = (side / 2.0) - margin
        stroke = max(5.0, side * 0.11)
        arc_rect = QRectF(cx - radius, cy - radius, radius * 2.0, radius * 2.0)

        # 트랙
        bg_col = QColor(255, 255, 255, 25) if not self.is_alert else QColor(0, 0, 0, 60)
        painter.setPen(QPen(bg_col, stroke, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
        painter.drawArc(arc_rect, 0, 360 * 16)

        # 게이지 호
        if self.is_alert:
            gauge_col = QColor(239, 68, 68)
        elif self.value >= self.warn_threshold:
            gauge_col = QColor(245, 158, 11)
        else:
            gauge_col = QColor(56, 189, 248)

        painter.setPen(QPen(gauge_col, stroke, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
        ratio = max(0.0, min(1.0, getattr(self, 'arc_ratio', self.value / 100.0)))
        span_angle = int(-ratio * 360 * 16)
        painter.drawArc(arc_rect, 90 * 16, span_angle)

        # 값 및 라벨
        painter.setPen(QColor(255, 255, 255))
        font_size_val = max(8, int(side * 0.13))
        font_size_sub = max(6, int(side * 0.08))

        painter.setFont(QFont("Segoe UI", font_size_val, QFont.Weight.Bold))
        fm_val = painter.fontMetrics()
        val_y = cy + (fm_val.ascent() / 3.0) - (font_size_sub * 0.4)
        painter.drawText(int(cx - (fm_val.horizontalAdvance(self.display_text) / 2)), int(val_y), self.display_text)

        painter.setFont(QFont("Segoe UI", font_size_sub))
        painter.setPen(QColor(203, 213, 225))
        fm_sub = painter.fontMetrics()
        sub_y = val_y + fm_sub.height()
        painter.drawText(int(cx - (fm_sub.horizontalAdvance(self.title) / 2)), int(sub_y), self.title)


# =====================================================================
# 4. 숫자/문자열 정렬 지원 TableWidgetItem
# =====================================================================
class NumericTableWidgetItem(QTableWidgetItem):
    def __init__(self, value, is_numeric=False):
        super().__init__(str(value))
        self.is_numeric = is_numeric
        self.numeric_value = float(value) if is_numeric else 0.0

    def __lt__(self, other):
        if self.is_numeric and isinstance(other, NumericTableWidgetItem) and other.is_numeric:
            return self.numeric_value < other.numeric_value
        return super().__lt__(other)


# =====================================================================
# 5. 디스크 상세 도넛 모달 다이얼로그 (수정사항 2)
# =====================================================================
class MultiDiskModal(QDialog):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("시스템 하드디스크 볼륨별 사용 현황")
        self.resize(650, 360)
        self.setStyleSheet("background-color: #0b192c; color: #fff;")

        layout = QVBoxLayout(self)
        title_lbl = QLabel("전체 드라이브 종류 및 사용량 (스토리지 실시간 분석)")
        title_lbl.setFont(QFont("Segoe UI", 11, QFont.Weight.Bold))
        title_lbl.setStyleSheet("color: #38bdf8; margin-bottom: 8px;")
        layout.addWidget(title_lbl)

        grid = QGridLayout()
        partitions = psutil.disk_partitions(all=False)
        col = 0
        row = 0

        for part in partitions:
            drive_mount = part.mountpoint
            try:
                usage = psutil.disk_usage(drive_mount)
                total_gb = round(usage.total / (1024**3), 1)
                used_gb = round(usage.used / (1024**3), 1)
                free_gb = round(usage.free / (1024**3), 1)
                pct = usage.percent

                container = QWidget()
                c_lay = QVBoxLayout(container)
                c_lay.setContentsMargins(4, 4, 4, 4)

                gauge = DonutGaugeWidget(f"{part.device}", "%")
                gauge.set_data(pct, is_alert=(pct >= 90), threshold=90.0)
                gauge.setMinimumSize(120, 120)

                info_lbl = QLabel(f"종류: {part.fstype or 'NTFS'}\n전체: {total_gb} GB\n사용: {used_gb} GB\n잔여: {free_gb} GB")
                info_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
                info_lbl.setStyleSheet("font-size: 11px; color: #cbd5e1;")

                c_lay.addWidget(gauge)
                c_lay.addWidget(info_lbl)
                grid.addWidget(container, row, col)

                col += 1
                if col >= 3:
                    col = 0
                    row += 1
            except Exception:
                continue

        layout.addLayout(grid)
        btn_close = QPushButton("닫기")
        btn_close.setStyleSheet("background-color: #1e3e62; padding: 6px; border-radius: 4px;")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close)


# =====================================================================
# 6. 프로세스 상세 실행 명령줄 모달 (수정사항 3)
# =====================================================================
class ProcessDetailModal(QDialog):
    def __init__(self, pid, p_name, parent=None):
        super().__init__(parent)
        self.setWindowTitle(f"프로세스 실행 정보 분석 - PID: {pid}")
        self.resize(600, 380)
        self.setStyleSheet("background-color: #0b192c; color: #fff;")

        layout = QVBoxLayout(self)

        title = QLabel(f"대상 프로세스: {p_name} (PID: {pid})")
        title.setFont(QFont("Segoe UI", 11, QFont.Weight.Bold))
        title.setStyleSheet("color: #38bdf8;")
        layout.addWidget(title)

        try:
            proc = psutil.Process(pid)
            exe_path = proc.exe() or "접근 권한 없음 (SYSTEM 권한 필요)"
            cmdline_list = proc.cmdline() or []
            cmdline = " ".join(cmdline_list) if cmdline_list else "명령줄 인수 없음 또는 접근 불가"
            user_name = proc.username() if hasattr(proc, 'username') else "Unknown"
            create_time = datetime.datetime.fromtimestamp(proc.create_time()).strftime("%Y-%m-%d %H:%M:%S")
        except Exception as e:
            exe_path = "종료되었거나 접근할 수 없습니다."
            cmdline = str(e)
            user_name = "-"
            create_time = "-"

        form = QFormLayout()
        lbl_exe = QLineEdit(exe_path)
        lbl_exe.setReadOnly(True)
        lbl_exe.setStyleSheet("background: #050b14; color: #a5f3fc;")

        lbl_user = QLineEdit(user_name)
        lbl_user.setReadOnly(True)
        lbl_user.setStyleSheet("background: #050b14; color: #a5f3fc;")

        lbl_time = QLineEdit(create_time)
        lbl_time.setReadOnly(True)
        lbl_time.setStyleSheet("background: #050b14; color: #a5f3fc;")

        form.addRow("실행 파일 경로(EXE):", lbl_exe)
        form.addRow("실행 계정:", lbl_user)
        form.addRow("구동 시간:", lbl_time)
        layout.addLayout(form)

        layout.addWidget(QLabel("실제 실행 명령줄 파라미터 (Full CommandLine / svchost Service):"))
        txt_cmd = QTextEdit()
        txt_cmd.setReadOnly(True)
        txt_cmd.setPlainText(cmdline)
        txt_cmd.setStyleSheet("background: #050b14; color: #facc15; font-family: 'Consolas'; font-size: 11px;")
        layout.addWidget(txt_cmd)

        btn_close = QPushButton("확인")
        btn_close.setStyleSheet("background-color: #0284c7; padding: 6px; font-weight: bold;")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close)


# =====================================================================
# 7. 사운드 알람 워커
# =====================================================================
class SoundAlarmWorker:
    def __init__(self):
        self._is_active = False
        self._thread = None
        self.enabled = True
        self.freq = 1200
        self.duration = 350

    def start_beep(self):
        if not self.enabled or self._is_active:
            return
        self._is_active = True
        self._thread = threading.Thread(target=self._run_beep, daemon=True)
        self._thread.start()

    def stop_beep(self):
        self._is_active = False

    def _run_beep(self):
        while self._is_active:
            if winsound and self.enabled:
                try:
                    winsound.Beep(self.freq, self.duration)
                except Exception:
                    pass
            time.sleep(1.0)


# =====================================================================
# 8. 메인 윈도우
# =====================================================================
class CheckServerMainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Studio Airbook - FAS & EAI Server Guard")
        self.resize(400, 400)
        self.setMinimumSize(380, 380)

        self.cfg = ConfigManager.load()
        self.is_warning_state = False
        self.sound_worker = SoundAlarmWorker()
        self.raw_logs = []
        self.bat_worker = None
        self.cpu_cores = psutil.cpu_count(logical=True) or 1

        self.PERM_LOG_FILE = r"C:\checkPMI\logs\system_accumulated.log"

        self._apply_cfg_to_runtime()
        self._init_ui()
        self._setup_timer()
        self.append_log("INFO", f"관제 엔진 구동 (논리 코어수: {self.cpu_cores}개)")

    def _apply_cfg_to_runtime(self):
        sys_cfg = self.cfg.get("system", {})
        self.sound_worker.enabled = sys_cfg.get("sound_alert_enabled", True)
        self.sound_worker.freq = sys_cfg.get("beep_freq_hz", 1200)
        self.sound_worker.duration = sys_cfg.get("beep_duration_ms", 350)
        self.TIME_SYNC_CMD = sys_cfg.get("time_sync_cmd", "w32tm /resync /force")

        th_cfg = self.cfg.get("thresholds", {})
        self.LIMIT_CPU = th_cfg.get("cpu_percent", 85)
        self.LIMIT_RAM = th_cfg.get("ram_percent", 85)
        self.LIMIT_DISK = th_cfg.get("disk_percent", 90)
        self.DISK_DRIVE = th_cfg.get("disk_drive", "C:\\")
        self.LIMIT_PROCS = th_cfg.get("proc_count_limit", 300)

        self.target_processes = self.cfg.get("target_processes", [])
        self.bat_paths = self.cfg.get("bat_paths", {})

    def _init_ui(self):
        self.central_widget = QWidget(self)
        self.setCentralWidget(self.central_widget)
        self.main_layout = QVBoxLayout(self.central_widget)
        self.main_layout.setContentsMargins(8, 8, 8, 8)
        self.main_layout.setSpacing(6)

        # 헤더
        header = QHBoxLayout()
        self.status_badge = QLabel("SYSTEM NORMAL")
        self.status_badge.setFont(QFont("Segoe UI", 9, QFont.Weight.Bold))
        self.status_badge.setStyleSheet("background-color: #0284c7; color: #fff; padding: 3px 6px; border-radius: 4px;")

        self.btn_resize = QPushButton("1080p")
        self.btn_resize.setFixedWidth(55)
        self.btn_resize.clicked.connect(self.toggle_1080p)

        self.btn_fullscreen = QPushButton("전체화면")
        self.btn_fullscreen.setFixedWidth(65)
        self.btn_fullscreen.clicked.connect(self.toggle_fullscreen)

        header.addWidget(self.status_badge)
        header.addStretch()
        header.addWidget(self.btn_resize)
        header.addWidget(self.btn_fullscreen)
        self.main_layout.addLayout(header)

        # 탭 컨트롤
        self.tabs = QTabWidget()
        self.tabs.setStyleSheet("""
            QTabWidget::pane { border: 1px solid rgba(255,255,255,0.15); border-radius: 4px; }
            QTabBar::tab { background: rgba(0,0,0,0.3); color: #94a3b8; padding: 5px 8px; font-weight: bold; font-size: 11px; }
            QTabBar::tab:selected { background: rgba(255,255,255,0.18); color: #fff; }
        """)

        # Tab 1: 대시보드
        self.tab_dash = QWidget()
        dash_layout = QVBoxLayout(self.tab_dash)
        gauges_grid = QHBoxLayout()

        self.gauge_cpu = DonutGaugeWidget("CPU", "%")
        self.gauge_ram = DonutGaugeWidget("RAM", "%")
        self.gauge_disk = DonutGaugeWidget("DISK", "%")
        self.gauge_proc = DonutGaugeWidget("PROCESS", "EA")

        # 수정사항 2 인터랙티브 클릭 연결
        self.gauge_cpu.clicked.connect(lambda: self.switch_proc_tab(sort_col=2))
        self.gauge_ram.clicked.connect(lambda: self.switch_proc_tab(sort_col=3))
        self.gauge_disk.clicked.connect(self.open_disk_modal)
        self.gauge_proc.clicked.connect(lambda: self.switch_proc_tab(sort_col=1))

        gauges_grid.addWidget(self.gauge_cpu)
        gauges_grid.addWidget(self.gauge_ram)
        gauges_grid.addWidget(self.gauge_disk)
        gauges_grid.addWidget(self.gauge_proc)
        dash_layout.addLayout(gauges_grid)

        self.alert_summary_lbl = QLabel("전체 프로세스 및 자원 정상 감시 중")
        self.alert_summary_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.alert_summary_lbl.setStyleSheet("color: #e2e8f0; font-size: 11px;")
        dash_layout.addWidget(self.alert_summary_lbl)
        self.tabs.addTab(self.tab_dash, "대시보드")

        # Tab 2: 실시간 프로세스
        self.tab_proc = QWidget()
        proc_layout = QVBoxLayout(self.tab_proc)

        proc_search_bar = QHBoxLayout()
        self.proc_search_edit = QLineEdit()
        self.proc_search_edit.setPlaceholderText("프로세스명 또는 PID 필터 검색...")
        self.proc_search_edit.textChanged.connect(self._filter_process_table)
        proc_search_bar.addWidget(QLabel("검색:"))
        proc_search_bar.addWidget(self.proc_search_edit)
        proc_layout.addLayout(proc_search_bar)

        self.proc_table = QTableWidget(0, 5)
        self.proc_table.setHorizontalHeaderLabels(["PID", "프로세스명", "CPU (%)", "Memory (MB)", "구분"])
        self.proc_table.setSortingEnabled(True)
        self.proc_table.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.Interactive)
        self.proc_table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        self.proc_table.setStyleSheet("background: #050b14; color: #fff; gridline-color: #1e293b;")
        # 수정사항 3: 더블클릭 시 상세 모달 호출
        self.proc_table.cellDoubleClicked.connect(self.on_process_row_clicked)
        proc_layout.addWidget(self.proc_table)
        self.tabs.addTab(self.tab_proc, "프로세스")

        # Tab 3: 점검 BAT 실행기
        self.tab_bat = QWidget()
        bat_layout = QVBoxLayout(self.tab_bat)
        btn_box = QHBoxLayout()
        self.btn_run_daily = QPushButton("일일 FAS 점검 (.bat)")
        self.btn_run_daily.setStyleSheet("background-color: #0284c7; font-weight: bold; padding: 6px;")
        self.btn_run_daily.clicked.connect(lambda: self.run_batch_task("daily_fas"))

        self.btn_run_eai = QPushButton("일일 EAI 점검 (.bat)")
        self.btn_run_eai.clicked.connect(lambda: self.run_batch_task("daily_eai"))

        btn_box.addWidget(self.btn_run_daily)
        btn_box.addWidget(self.btn_run_eai)
        bat_layout.addLayout(btn_box)

        self.bat_console = QTextEdit()
        self.bat_console.setReadOnly(True)
        self.bat_console.setStyleSheet("background-color: #050b14; color: #a5f3fc; font-family: 'Consolas'; font-size: 11px;")
        bat_layout.addWidget(self.bat_console)
        self.tabs.addTab(self.tab_bat, "점검실행")

        # Tab 4: 결과 뷰어
        self._build_result_viewer_tab()

        # Tab 5: 누적 로그 열람기
        self.tab_logs = QWidget()
        logs_layout = QVBoxLayout(self.tab_logs)
        l_top = QHBoxLayout()
        self.log_filter_edit = QLineEdit()
        self.log_filter_edit.setPlaceholderText("로그 검색...")
        self.log_filter_edit.textChanged.connect(self._filter_logs)
        btn_reload_log = QPushButton("누적파일 재동기화")
        btn_reload_log.clicked.connect(self._load_accumulated_logs)
        l_top.addWidget(self.log_filter_edit)
        l_top.addWidget(btn_reload_log)
        logs_layout.addLayout(l_top)

        self.log_viewer = QTextEdit()
        self.log_viewer.setReadOnly(True)
        self.log_viewer.setStyleSheet("background-color: #050b14; color: #38bdf8; font-family: 'Consolas'; font-size: 11px;")
        logs_layout.addWidget(self.log_viewer)
        self.tabs.addTab(self.tab_logs, "로그")

        # Tab 6: 환경설정
        self._build_settings_tab()

        self.main_layout.addWidget(self.tabs)

        # 푸터
        self.footer = QLabel("© 2026 Studio Airbook. All rights reserved.")
        self.footer.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.footer.setStyleSheet("color: rgba(255,255,255,0.4); font-size: 9px;")
        self.main_layout.addWidget(self.footer)

        self._apply_normal_theme()
        self._load_accumulated_logs()

    # =================================================================
    # 9. 상호작용 및 모달 호출 (수정사항 2, 3)
    # =================================================================
    def switch_proc_tab(self, sort_col):
        self.tabs.setCurrentWidget(self.tab_proc)
        self.proc_table.sortItems(sort_col, Qt.SortOrder.DescendingOrder)

    def open_disk_modal(self):
        modal = MultiDiskModal(self)
        modal.exec()

    def on_process_row_clicked(self, row, col):
        pid_item = self.proc_table.item(row, 0)
        name_item = self.proc_table.item(row, 1)
        if pid_item and name_item:
            try:
                pid = int(pid_item.text())
                p_name = name_item.text()
                modal = ProcessDetailModal(pid, p_name, self)
                modal.exec()
            except Exception:
                pass

    # =================================================================
    # 10. 결과 뷰어 탭
    # =================================================================
    def _build_result_viewer_tab(self):
        self.tab_result = QWidget()
        res_layout = QVBoxLayout(self.tab_result)
        res_layout.setContentsMargins(6, 6, 6, 6)

        top_container = QWidget()
        top_layout = QHBoxLayout(top_container)
        top_layout.setContentsMargins(0, 0, 0, 0)

        self.cal_widget = QCalendarWidget()
        self.cal_widget.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        self.cal_widget.setStyleSheet("""
            QCalendarWidget QWidget { background-color: #0f172a; color: #fff; }
            QCalendarWidget QAbstractItemView:enabled { color: #fff; selection-background-color: #0284c7; }
        """)
        self.cal_widget.selectionChanged.connect(self.on_date_selected)

        time_box = QVBoxLayout()
        time_box.addWidget(QLabel("점검 이력 시간대 (타임스탬프):"))
        self.combo_timestamps = QComboBox()
        self.combo_timestamps.currentIndexChanged.connect(self.load_selected_result)
        time_box.addWidget(self.combo_timestamps)

        self.btn_refresh_dates = QPushButton("이력 새로고침")
        self.btn_refresh_dates.clicked.connect(self.on_date_selected)
        time_box.addWidget(self.btn_refresh_dates)
        time_box.addStretch()

        top_layout.addWidget(self.cal_widget, 4)
        top_layout.addLayout(time_box, 6)
        res_layout.addWidget(top_container, 3)

        self.result_splitter = QSplitter(Qt.Orientation.Horizontal)
        self.txt_result_view = QTextEdit()
        self.txt_result_view.setReadOnly(True)
        self.txt_result_view.setStyleSheet("background: #050b14; color: #f1f5f9; font-family: 'Consolas'; font-size: 11px;")

        self.csv_result_table = QTableWidget(0, 8)
        self.csv_result_table.setHorizontalHeaderLabels(["구분", "주기", "장치", "항목", "대상", "순번", "상태", "비고"])
        self.csv_result_table.setSortingEnabled(True)
        self.csv_result_table.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.Interactive)
        self.csv_result_table.setStyleSheet("background: #050b14; color: #fff; gridline-color: #1e293b;")

        self.result_splitter.addWidget(self.txt_result_view)
        self.result_splitter.addWidget(self.csv_result_table)
        self.result_splitter.setSizes([450, 750])

        res_layout.addWidget(self.result_splitter, 7)
        self.tabs.addTab(self.tab_result, "결과조회")
        QTimer.singleShot(500, self.on_date_selected)

    def on_date_selected(self):
        sel_date = self.cal_widget.selectedDate().toString("yyyyMMdd")
        logs_root = r"C:\checkPMI\logs"
        self.combo_timestamps.clear()

        if not os.path.exists(logs_root):
            return

        matched_dirs = []
        try:
            for d in os.listdir(logs_root):
                if d.startswith(sel_date) and os.path.isdir(os.path.join(logs_root, d)):
                    matched_dirs.append(d)
        except Exception:
            pass

        matched_dirs.sort(reverse=True)
        for md in matched_dirs:
            self.combo_timestamps.addItem(md)

        if not matched_dirs:
            self.txt_result_view.setText("해당 일자에 실행된 점검 로그가 없습니다.")
            self.csv_result_table.setRowCount(0)

    def load_selected_result(self):
        ts = self.combo_timestamps.currentText()
        if not ts:
            return

        target_dir = os.path.join(r"C:\checkPMI\logs", ts)
        txt_path = os.path.join(target_dir, "check_result.txt")
        csv_path = os.path.join(target_dir, "check_result.csv")

        if os.path.exists(txt_path):
            try:
                with open(txt_path, "r", encoding="utf-8", errors="replace") as f:
                    self.txt_result_view.setText(f.read())
            except Exception:
                with open(txt_path, "r", encoding="cp949", errors="replace") as f:
                    self.txt_result_view.setText(f.read())
        else:
            self.txt_result_view.setText(f"check_result.txt 없음: {txt_path}")

        self.csv_result_table.setSortingEnabled(False)
        self.csv_result_table.setRowCount(0)
        if os.path.exists(csv_path):
            try:
                with open(csv_path, "r", encoding="utf-8", errors="replace") as f:
                    rows = list(csv.reader(f))
            except Exception:
                with open(csv_path, "r", encoding="cp949", errors="replace") as f:
                    rows = list(csv.reader(f))

            if len(rows) > 1:
                data_rows = rows[1:]
                self.csv_result_table.setRowCount(len(data_rows))
                for r_idx, row in enumerate(data_rows):
                    for c_idx, val in enumerate(row):
                        if c_idx < 8:
                            item = QTableWidgetItem(val.strip())
                            if val.strip() == "PASS":
                                item.setForeground(QColor(56, 189, 248))
                            elif val.strip() in ["FAIL", "ERR"]:
                                item.setForeground(QColor(239, 68, 68))
                            elif val.strip() in ["WARN", "CHECK"]:
                                item.setForeground(QColor(245, 158, 11))
                            self.csv_result_table.setItem(r_idx, c_idx, item)
        self.csv_result_table.setSortingEnabled(True)

    # =================================================================
    # 11. 배치 실행 컨트롤러
    # =================================================================
    def run_batch_task(self, key):
        bat_file = self.bat_paths.get(key, "")
        if not os.path.exists(bat_file):
            ConfigManager.ensure_default_bats()

        if not os.path.exists(bat_file):
            QMessageBox.warning(self, "파일 부재", f"배치 스크립트를 생성할 수 없습니다: {bat_file}")
            return

        self.btn_run_daily.setEnabled(False)
        self.btn_run_eai.setEnabled(False)
        self.bat_console.clear()
        self.tabs.setCurrentWidget(self.tab_bat)

        now_ts = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
        target_dir = os.path.join(r"C:\checkPMI\logs", now_ts)

        self.bat_worker = BatRunnerThread(bat_file, target_dir)
        self.bat_worker.output_signal.connect(self.bat_console.append)
        self.bat_worker.finished_signal.connect(self._on_bat_finished)
        self.bat_worker.start()

    def _on_bat_finished(self, success, target_dir):
        self.btn_run_daily.setEnabled(True)
        self.btn_run_eai.setEnabled(True)
        if success:
            self.append_log("INSPECT", "점검 배치 완료 (TXT/CSV 결과 저장)")
            self.on_date_selected()
        else:
            self.append_log("ERR", "배치 실행 중 오류 감지")

    # =================================================================
    # 12. 실시간 모니터링 엔진
    # =================================================================
    def _setup_timer(self):
        self.monitor_timer = QTimer(self)
        self.monitor_timer.timeout.connect(self.run_system_inspection)
        interval = self.cfg.get("system", {}).get("interval_sec", 2) * 1000
        self.monitor_timer.start(interval)

    def run_system_inspection(self):
        raw_cpu = psutil.cpu_percent(interval=None)
        cpu_usage = min(100.0, max(0.0, raw_cpu))
        ram_usage = psutil.virtual_memory().percent
        try:
            disk_usage = psutil.disk_usage(self.DISK_DRIVE).percent
        except Exception:
            disk_usage = 0.0

        procs_info = []
        target_set = {t.lower(): t for t in self.target_processes}
        found_targets = set()

        for p in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_info']):
            try:
                p_name = p.info['name'] or "System"
                p_pid = p.info['pid']

                raw_proc_cpu = p.info['cpu_percent'] or 0.0
                norm_proc_cpu = round(raw_proc_cpu / self.cpu_cores, 1)

                mem_bytes = p.info['memory_info'].rss if p.info['memory_info'] else 0
                p_mem_mb = round(mem_bytes / (1024 * 1024), 1)

                is_target = p_name.lower() in target_set
                if is_target:
                    found_targets.add(p_name.lower())

                procs_info.append({
                    'pid': p_pid,
                    'name': p_name,
                    'cpu': norm_proc_cpu,
                    'mem': p_mem_mb,
                    'is_target': is_target
                })
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                continue

        sorted_by_load = sorted(procs_info, key=lambda x: (x['cpu'] * 2.0 + (x['mem'] / 50.0)), reverse=True)
        top5_pids = [p['pid'] for p in sorted_by_load[:5]]

        self.proc_table.setSortingEnabled(False)
        self.proc_table.setRowCount(len(procs_info))

        cascade_colors = [
            QColor(185, 28, 28, 230),
            QColor(185, 28, 28, 190),
            QColor(185, 28, 28, 150),
            QColor(185, 28, 28, 110),
            QColor(185, 28, 28, 70),
        ]

        search_kw = self.proc_search_edit.text().strip().lower()

        for row, p in enumerate(procs_info):
            pid_item = NumericTableWidgetItem(p['pid'], is_numeric=True)
            name_item = QTableWidgetItem(p['name'])
            cpu_item = NumericTableWidgetItem(p['cpu'], is_numeric=True)
            mem_item = NumericTableWidgetItem(p['mem'], is_numeric=True)
            type_item = QTableWidgetItem("일반")

            if p['is_target']:
                type_item.setText("감시대상")
                name_item.setForeground(QColor(56, 189, 248))
                type_item.setForeground(QColor(56, 189, 248))
                font_bold = QFont()
                font_bold.setBold(True)
                name_item.setFont(font_bold)
                type_item.setFont(font_bold)

            if p['pid'] in top5_pids:
                rank = top5_pids.index(p['pid'])
                c_brush = QBrush(cascade_colors[rank])
                pid_item.setBackground(c_brush)
                name_item.setBackground(c_brush)
                cpu_item.setBackground(c_brush)
                mem_item.setBackground(c_brush)
                type_item.setBackground(c_brush)
                if not p['is_target']:
                    type_item.setText(f"Top {rank + 1}")

            self.proc_table.setItem(row, 0, pid_item)
            self.proc_table.setItem(row, 1, name_item)
            self.proc_table.setItem(row, 2, cpu_item)
            self.proc_table.setItem(row, 3, mem_item)
            self.proc_table.setItem(row, 4, type_item)

            if search_kw:
                match = (search_kw in p['name'].lower()) or (search_kw in str(p['pid']))
                self.proc_table.setRowHidden(row, not match)
            else:
                self.proc_table.setRowHidden(row, False)

        self.proc_table.setSortingEnabled(True)

        total_proc_count = len(procs_info)
        missing_targets = [t for low_t, t in target_set.items() if low_t not in found_targets]

        # 수정사항 1: 설정된 프로세스 수량 초과 시 경보 판별
        has_proc_count_alert = (total_proc_count >= self.LIMIT_PROCS)
        has_res_alert = (cpu_usage >= self.LIMIT_CPU or ram_usage >= self.LIMIT_RAM or disk_usage >= self.LIMIT_DISK or has_proc_count_alert)
        has_proc_missing_alert = len(missing_targets) > 0
        current_alert = has_res_alert or has_proc_missing_alert

        self.gauge_cpu.set_data(cpu_usage, is_alert=(cpu_usage >= self.LIMIT_CPU), threshold=self.LIMIT_CPU)
        self.gauge_ram.set_data(ram_usage, is_alert=(ram_usage >= self.LIMIT_RAM), threshold=self.LIMIT_RAM)
        self.gauge_disk.set_data(disk_usage, is_alert=(disk_usage >= self.LIMIT_DISK), threshold=self.LIMIT_DISK)
        self.gauge_proc.set_data(
            value=total_proc_count,
            is_alert=has_proc_count_alert or has_proc_missing_alert,
            threshold=float(self.LIMIT_PROCS),
            custom_text=f"{total_proc_count}EA",
            arc_ratio=min(1.0, total_proc_count / float(self.LIMIT_PROCS or 300))
        )

        if current_alert:
            reasons = []
            if missing_targets:
                reasons.append(f"감시 프로세스 부재({', '.join(missing_targets)})")
            if has_proc_count_alert:
                reasons.append(f"프로세스 수량 초과({total_proc_count}EA >= {self.LIMIT_PROCS}EA)")
            if cpu_usage >= self.LIMIT_CPU:
                reasons.append(f"CPU 초과({cpu_usage:.1f}%)")
            if ram_usage >= self.LIMIT_RAM:
                reasons.append(f"RAM 초과({ram_usage:.1f}%)")
            if disk_usage >= self.LIMIT_DISK:
                reasons.append(f"DISK 초과({disk_usage:.1f}%)")

            msg = " / ".join(reasons)
            self.alert_summary_lbl.setText(f"[경고] {msg}")

            if not self.is_warning_state:
                self.is_warning_state = True
                self._apply_alert_theme()
                self.sound_worker.start_beep()
                self.append_log("WARN", msg)
        else:
            self.alert_summary_lbl.setText(f"정상 감시 중 (실행 프로세스: {total_proc_count}개)")
            if self.is_warning_state:
                self.is_warning_state = False
                self._apply_normal_theme()
                self.sound_worker.stop_beep()
                self.append_log("RECOVER", "모든 시스템 지표가 정상으로 복귀되었습니다.")

    def _filter_process_table(self, text):
        kw = text.strip().lower()
        for row in range(self.proc_table.rowCount()):
            name_item = self.proc_table.item(row, 1)
            pid_item = self.proc_table.item(row, 0)
            if name_item and pid_item:
                match = (kw in name_item.text().lower()) or (kw in pid_item.text())
                self.proc_table.setRowHidden(row, not match)

    # =================================================================
    # 13. 환경설정 탭 (수정사항 4: 드라이브 콤보 & 시간동기화)
    # =================================================================
    def _build_settings_tab(self):
        self.tab_settings = QWidget()
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setStyleSheet("background: transparent; border: none;")

        inner = QWidget()
        layout = QVBoxLayout(inner)
        layout.setSpacing(12)

        # 프로세스 목록
        grp_proc = QGroupBox("감시 대상 프로세스 목록 (줄바꿈 구분)")
        g_l = QVBoxLayout(grp_proc)
        self.edit_procs = QTextEdit()
        self.edit_procs.setPlainText("\n".join(self.target_processes))
        self.edit_procs.setMinimumHeight(90)
        self.edit_procs.setStyleSheet("background: #050b14; color: #fff;")
        g_l.addWidget(self.edit_procs)
        layout.addWidget(grp_proc)

        # 리소스 임계치 및 프로세스 수량 한도
        grp_th = QGroupBox("리소스 경고 임계치 및 감시 드라이브")
        th_f = QFormLayout(grp_th)
        self.sp_cpu = QSpinBox()
        self.sp_cpu.setRange(10, 100)
        self.sp_cpu.setValue(self.LIMIT_CPU)

        self.sp_ram = QSpinBox()
        self.sp_ram.setRange(10, 100)
        self.sp_ram.setValue(self.LIMIT_RAM)

        self.sp_disk = QSpinBox()
        self.sp_disk.setRange(10, 100)
        self.sp_disk.setValue(self.LIMIT_DISK)

        # 수정사항 1: 프로세스 수량 경보 설정
        self.sp_proc_limit = QSpinBox()
        self.sp_proc_limit.setRange(50, 2000)
        self.sp_proc_limit.setValue(self.LIMIT_PROCS)

        # 수정사항 4: 시스템 감시 드라이브 선택 콤보박스
        self.combo_drive = QComboBox()
        active_partitions = [p.mountpoint for p in psutil.disk_partitions(all=False)]
        for drive in active_partitions:
            self.combo_drive.addItem(drive)
        idx = self.combo_drive.findText(self.DISK_DRIVE)
        if idx >= 0:
            self.combo_drive.setCurrentIndex(idx)

        th_f.addRow("CPU (%):", self.sp_cpu)
        th_f.addRow("RAM (%):", self.sp_ram)
        th_f.addRow("DISK (%):", self.sp_disk)
        th_f.addRow("프로세스 수량 경보 기준(EA):", self.sp_proc_limit)
        th_f.addRow("기본 감시 드라이브:", self.combo_drive)
        layout.addWidget(grp_th)

        # 수정사항 4: 시간 동기화 설정 및 수동 트리거
        grp_time = QGroupBox("NTP 서버 시간 동기화 설정")
        t_f = QFormLayout(grp_time)
        self.ed_time_cmd = QLineEdit(self.TIME_SYNC_CMD)
        t_f.addRow("시간 동기화 명령어:", self.ed_time_cmd)

        btn_sync_now = QPushButton("지금 즉시 시간 동기화 실행 (NTP Resync)")
        btn_sync_now.setStyleSheet("background-color: #0d9488; font-weight: bold; padding: 6px;")
        btn_sync_now.clicked.connect(self._run_time_sync)
        t_f.addRow("", btn_sync_now)
        layout.addWidget(grp_time)

        # 배치 경로
        grp_bat = QGroupBox("점검 배치 스크립트 경로 (.bat)")
        b_f = QFormLayout(grp_bat)
        self.ed_bat_daily = QLineEdit(self.bat_paths.get("daily_fas", ""))
        self.ed_bat_eai = QLineEdit(self.bat_paths.get("daily_eai", ""))
        b_f.addRow("FAS 일일점검 BAT:", self.ed_bat_daily)
        b_f.addRow("EAI 일일점검 BAT:", self.ed_bat_eai)
        layout.addWidget(grp_bat)

        btn_save = QPushButton("설정 저장 (userset.json 즉시 반영)")
        btn_save.setStyleSheet("background-color: #0284c7; font-weight: bold; padding: 8px;")
        btn_save.clicked.connect(self._save_settings)
        layout.addWidget(btn_save)

        scroll.setWidget(inner)
        v = QVBoxLayout(self.tab_settings)
        v.addWidget(scroll)
        self.tabs.addTab(self.tab_settings, "설정")

    def _run_time_sync(self):
        cmd = self.ed_time_cmd.text().strip()
        self.append_log("CMD", f"시간 동기화 요청 실행: {cmd}")
        try:
            res = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=10)
            msg = res.stdout.strip() or res.stderr.strip() or "명령이 실행되었습니다."
            self.append_log("TIME", f"동기화 결과: {msg}")
            QMessageBox.information(self, "시간 동기화", f"동기화 명령 실행 결과:\n{msg}")
        except Exception as e:
            self.append_log("ERR", f"시간 동기화 실패: {str(e)}")
            QMessageBox.critical(self, "오류", f"시간 동기화 실패:\n{str(e)}")

    def _save_settings(self):
        new_procs = [p.strip() for p in self.edit_procs.toPlainText().splitlines() if p.strip()]
        new_cfg = {
            "system": {
                "interval_sec": self.cfg.get("system", {}).get("interval_sec", 2),
                "sound_alert_enabled": self.cfg.get("system", {}).get("sound_alert_enabled", True),
                "beep_freq_hz": self.cfg.get("system", {}).get("beep_freq_hz", 1200),
                "beep_duration_ms": 350,
                "time_sync_cmd": self.ed_time_cmd.text().strip()
            },
            "thresholds": {
                "cpu_percent": self.sp_cpu.value(),
                "ram_percent": self.sp_ram.value(),
                "disk_percent": self.sp_disk.value(),
                "disk_drive": self.combo_drive.currentText().strip(),
                "proc_count_limit": self.sp_proc_limit.value()
            },
            "target_processes": new_procs,
            "bat_paths": {
                "daily_fas": self.ed_bat_daily.text().strip(),
                "daily_eai": self.ed_bat_eai.text().strip(),
                "monthly": self.bat_paths.get("monthly", ""),
                "annual": self.bat_paths.get("annual", "")
            }
        }
        if ConfigManager.save(new_cfg):
            self.cfg = new_cfg
            self._apply_cfg_to_runtime()
            self.append_log("CONFIG", "userset.json 설정 저장 및 반영 완료")
            QMessageBox.information(self, "저장 완료", "환경설정이 저장되었습니다.")

    # =================================================================
    # 14. 영구 누적 로그 관리 (수정사항 3)
    # =================================================================
    def append_log(self, level, message):
        now_str = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        entry = f"[{now_str}] [{level:<7}] {message}"
        self.raw_logs.append(entry)
        self.log_viewer.append(entry)

        # 영구 누적 파일에 기록
        try:
            log_dir = os.path.dirname(self.PERM_LOG_FILE)
            if not os.path.exists(log_dir):
                os.makedirs(log_dir, exist_ok=True)
            with open(self.PERM_LOG_FILE, "a", encoding="utf-8") as f:
                f.write(entry + "\n")
        except Exception:
            pass

    def _load_accumulated_logs(self):
        if os.path.exists(self.PERM_LOG_FILE):
            try:
                with open(self.PERM_LOG_FILE, "r", encoding="utf-8", errors="replace") as f:
                    lines = [line.strip() for line in f if line.strip()]
                    self.raw_logs = lines[-1000:] # 최근 1000줄 메모리 유지
                    self.log_viewer.setPlainText("\n".join(self.raw_logs))
                    self.log_viewer.verticalScrollBar().setValue(self.log_viewer.verticalScrollBar().maximum())
            except Exception:
                pass

    def _filter_logs(self, text):
        filtered = [l for l in self.raw_logs if text.lower() in l.lower()]
        self.log_viewer.setPlainText("\n".join(filtered))

    # =================================================================
    # 15. UI 공통 테마 및 윈도우 크기
    # =================================================================
    def _apply_normal_theme(self):
        self.setStyleSheet("""
            QWidget { background-color: #0b192c; color: #fff; font-family: 'Segoe UI', 'Malgun Gothic'; }
            QPushButton { background-color: #1e3e62; border: 1px solid #2e5984; border-radius: 4px; color: #fff; padding: 4px; }
            QPushButton:hover { background-color: #2b5680; }
            QLineEdit, QSpinBox, QComboBox { background-color: #050b14; color: #fff; border: 1px solid #334155; padding: 3px; }
            QGroupBox { border: 1px solid #334155; border-radius: 4px; margin-top: 8px; font-weight: bold; }
            QGroupBox::title { subcontrol-origin: margin; left: 8px; padding: 0 4px; color: #38bdf8; }
        """)
        self.status_badge.setText("SYSTEM NORMAL")
        self.status_badge.setStyleSheet("background-color: #0284c7; color: #fff; padding: 3px 6px; border-radius: 4px;")

    def _apply_alert_theme(self):
        self.setStyleSheet("""
            QWidget { background-color: #7f1d1d; color: #fff; font-family: 'Segoe UI', 'Malgun Gothic'; }
            QPushButton { background-color: #991b1b; border: 1px solid #ef4444; border-radius: 4px; color: #fff; padding: 4px; }
            QPushButton:hover { background-color: #b91c1c; }
            QLineEdit, QSpinBox, QComboBox { background-color: #450a0a; color: #fff; border: 1px solid #ef4444; padding: 3px; }
            QGroupBox { border: 1px solid #ef4444; border-radius: 4px; margin-top: 8px; font-weight: bold; }
            QGroupBox::title { subcontrol-origin: margin; left: 8px; padding: 0 4px; color: #fca5a5; }
        """)
        self.status_badge.setText("SYSTEM ALERT")
        self.status_badge.setStyleSheet("background-color: #dc2626; color: #fff; padding: 3px 6px; border-radius: 4px;")

    def toggle_1080p(self):
        if self.width() == 1920 and self.height() == 1080:
            self.resize(400, 400)
            self.btn_resize.setText("1080p")
        else:
            self.showNormal()
            self.resize(1920, 1080)
            self.btn_resize.setText("400p")

    def toggle_fullscreen(self):
        if self.isFullScreen():
            self.showNormal()
            self.resize(400, 400)
            self.btn_fullscreen.setText("전체화면")
        else:
            self.showFullScreen()
            self.btn_fullscreen.setText("창모드")

    def keyPressEvent(self, event):
        if event.key() == Qt.Key.Key_F11:
            self.toggle_fullscreen()
        else:
            super().keyPressEvent(event)

    def closeEvent(self, event):
        self.sound_worker.stop_beep()
        event.accept()


# =====================================================================
# 16. 엔트리포인트
# =====================================================================
if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = CheckServerMainWindow()
    window.show()
    sys.exit(app.exec())