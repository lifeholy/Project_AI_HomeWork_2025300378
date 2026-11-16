#!/usr/bin/env module
import sys
import os
import time
import serial
import threading
import csv
import random
from datetime import datetime, timedelta

# GUI 및 스타일 라이브러리
import tkinter as tk
from tkinter import ttk
import ttkbootstrap as tb
from ttkbootstrap.dialogs import Messagebox

# 그래픽 차트 라이브러리
import matplotlib.pyplot as plt
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg, NavigationToolbar2Tk
import matplotlib.animation as animation

# ----------------------------------------------------------------
# Matplotlib 한글 폰트 깨짐 방지 설정
# ----------------------------------------------------------------
import matplotlib
from matplotlib import font_manager, rc

if sys.platform == 'win32':
    # 윈도우일 경우 맑은 고딕(Malgun Gothic) 설정
    font_name = font_manager.FontProperties(fname="c:/Windows/Fonts/malgun.ttf").get_name()
    rc('font', family=font_name)
else:
    # 리눅스/라즈베리파이일 경우 Nanum 폰트가 있다면 설정 (없으면 기본 적용)
    rc('font', family='NanumGothic')

# 마이너스 기호(-) 깨짐 방지
matplotlib.rcParams['axes.unicode_minus'] = False

# ----------------------------------------------------------------
# 환경 설정
# ----------------------------------------------------------------
SERIAL_PORT = 'COM3' if sys.platform == 'win32' else '/dev/ttyUSB0'
BAUD_RATE = 9600
SECURE_DATA_DIR = './secure_data' if sys.platform == 'win32' else '/var/www/secure/data'

if not os.path.exists(SECURE_DATA_DIR):
    os.makedirs(SECURE_DATA_DIR)

class AdvancedFlowDashboard:
    def __init__(self, root):
        self.root = root
        self.root.title("Xonic-100 UltraSonic Flowmeter Premium Enterprise Dashboard")
        self.root.geometry("1440x900")
        
        # 고급 다크 모드 superhero 테마 설정
        self.style = tb.Style(theme="superhero")
        
        # 실시간 데이터 메모리 변수
        self.current_flow = 0.0
        self.current_total = 125.450
        self.current_velocity = 0.0
        self.error_code = "0000"
        
        # 실시간 차트 60초 버퍼
        self.flow_history = [0.0] * 60
        
        # 과거 이력 조회용 통합 버퍼 (차트-리스트 연동용)
        self.history_data_list = []  # [{timestamp, header, flow, total, velocity, error}, ...]
        self.is_history_mode = False # 현재 화면이 과거 이력 모드인지 여부
        
        # 스레드 제어 플래그
        self.running = True
        self.test_mode = False
        
        # 달력 날짜 제어용 변수 (초기값: 현재 달)
        self.cal_year = datetime.now().year
        self.cal_month = datetime.now().month
        
        # UI 구축
        self.setup_ui()
        
        # 백엔드 엔진 가동
        self.start_backend_services()

    def setup_ui(self):
        """좌우 분할 스케일 및 다이내믹 그래픽 프레임 레이아웃 배치"""
        # 메인 컨테이너 프레임
        main_container = tb.Frame(self.root, padding=15)
        main_container.pack(fill=tk.BOTH, expand=True)
        
        # ==========================================
        # 1. TOP BAR (타이틀 및 상단 컨트롤 제어부)
        # ==========================================
        top_bar = tb.Frame(main_container)
        top_bar.pack(fill=tk.X, side=tk.TOP, pady=(0, 10))
        
        title_lbl = tb.Label(top_bar, text="📊 Xonic-100 초음파 유량계 통합 관제 시스템", font=("Helvetica", 20, "bold"))
        title_lbl.pack(side=tk.LEFT)
        
        # [테스트] 토글 스위치 버튼
        self.test_btn = tb.Button(
            top_bar, text="⚙️ 시뮬레이션 TEST 시작", 
            bootstyle="outline-warning", command=self.toggle_test_mode, width=24
        )
        self.test_btn.pack(side=tk.RIGHT, padx=(15, 0))
        
        self.status_lbl = tb.Label(top_bar, text=f"연결 대기중 ({SERIAL_PORT})", font=("Helvetica", 11), bootstyle="warning")
        self.status_lbl.pack(side=tk.RIGHT, pady=5)

        # ==========================================
        # 2. BODY AREA (좌측 사이드바 + 우측 대형 관제창)
        # ==========================================
        body_frame = tb.Frame(main_container)
        body_frame.pack(fill=tk.BOTH, expand=True)
        
        # ------------------------------------------
        # 2-A. 좌측 사이드바 (실시간 수치 카드 + 가동 달력)
        # ------------------------------------------
        left_sidebar = tb.Frame(body_frame, width=360)
        left_sidebar.pack(fill=tk.Y, side=tk.LEFT, padx=(0, 15))
        left_sidebar.pack_propagate(False)
        
        # [카드 1] 순시 유량
        flow_card = tb.Labelframe(left_sidebar, text=" 순시 유량 (Flow Rate) ", padding=8, bootstyle="info")
        flow_card.pack(fill=tk.X, pady=(0, 8))
        self.flow_val_lbl = tb.Label(flow_card, text="0.000 m³/h", font=("Courier", 22, "bold"), bootstyle="info")
        self.flow_val_lbl.pack(anchor=tk.CENTER, pady=5)
        
        # [카드 2] 적산 유량
        total_card = tb.Labelframe(left_sidebar, text=" 적산 유량 (Totalizer) ", padding=8, bootstyle="success")
        total_card.pack(fill=tk.X, pady=8)
        self.total_val_lbl = tb.Label(total_card, text="125.450 m³", font=("Courier", 22, "bold"), bootstyle="success")
        self.total_val_lbl.pack(anchor=tk.CENTER, pady=5)
        
        # [카드 3] 물리 계수 서브 창
        sub_card = tb.Frame(left_sidebar)
        sub_card.pack(fill=tk.X, pady=4)
        self.vel_val_lbl = tb.Label(sub_card, text="유속: 0.00 m/s", font=("Helvetica", 12, "bold"), bootstyle="primary")
        self.vel_val_lbl.pack(side=tk.LEFT)
        self.err_val_lbl = tb.Label(sub_card, text="Error: 0000000000", font=("Helvetica", 11), bootstyle="secondary")
        self.err_val_lbl.pack(side=tk.RIGHT)
        
        # [과거 로그 달력 위젯 프레임]
        self.cal_frame = tb.Labelframe(left_sidebar, text=" 과거 이력 조회 달력 (Data Calendar) ", padding=5, bootstyle="primary")
        self.cal_frame.pack(fill=tk.BOTH, expand=True, pady=(8, 0))
        self.draw_calendar() # 달력 그리기 호출

        # ------------------------------------------
        # 2-B. 우측 메인 섹션 (상단 차트 + 하단 리스트뷰 테이블)
        # ------------------------------------------
        right_main = tb.Frame(body_frame)
        right_main.pack(fill=tk.BOTH, side=tk.RIGHT, expand=True)
        
        # [우측 상단 차트 프레임]
        chart_container = tb.Labelframe(right_main, text=" 유량 데이터 그래픽 분석 차트 (Interactive Flow Chart) ", padding=5, bootstyle="secondary")
        chart_container.pack(fill=tk.BOTH, expand=True, pady=(0, 10))
        
        # 과거 24시간 스크롤 및 줌/팬을 위한 툴바 장착용 프레임 분리
        self.toolbar_frame = tb.Frame(chart_container)
        self.toolbar_frame.pack(fill=tk.X, side=tk.TOP)
        
        plt.style.use('dark_background')
        self.fig, self.ax = plt.subplots(figsize=(7, 3.8), dpi=100)
        self.fig.patch.set_facecolor('#2b3e50')  # superhero 배경 동기화
        self.ax.set_facecolor('#1a252f')
        
        # 실시간용 라인 생성
        self.line, = self.ax.plot(self.flow_history, color='#00bc8c', linewidth=2.5, label='Flow Rate (m³/h)')
        self.ax.set_ylim(0, 50)
        self.ax.grid(True, color='#2c3e50', linestyle='--')
        self.ax.legend(loc='upper left')
        
        # 차트 클릭 마우스 이벤트 등록 (특정 시간대 매칭용)
        self.fig.canvas.mpl_connect('button_press_event', self.on_chart_click)
        
        self.canvas = FigureCanvasTkAgg(self.fig, master=chart_container)
        self.canvas.get_tk_widget().pack(fill=tk.BOTH, expand=True)
        
        # 상하좌우 자유 스크롤/줌인/확대를 위한 Matplotlib 표준 매체 내비게이션 툴바 탑재
        self.toolbar = NavigationToolbar2Tk(self.canvas, self.toolbar_frame)
        self.toolbar.update()
        
        # [우측 하단 상하좌우 양방향 스크롤 리스트 뷰] (height 위치 및 고정옵션 수정)
        list_container = tb.Labelframe(right_main, text=" 데이터 패키지 테이블 레코드 리스트 (Double-Scroll Log Table) ", padding=5, bootstyle="secondary", height=260)
        list_container.pack_propagate(False) # <--- 하위 리스트뷰 때문에 세로 크기가 찌그러지지 않도록 고정
        list_container.pack(fill=tk.X, side=tk.BOTTOM, pady=(10, 0))
        
        # 양방향 스크롤바 바인딩 구조
        vsb = tb.Scrollbar(list_container, orient=tk.VERTICAL)
        vsb.pack(side=tk.RIGHT, fill=tk.Y)
        hsb = tb.Scrollbar(list_container, orient=tk.HORIZONTAL)
        hsb.pack(side=tk.BOTTOM, fill=tk.X)
        
        columns = ('timestamp', 'header', 'flow', 'total', 'velocity', 'error')
        self.tree = ttk.Treeview(list_container, columns=columns, show='headings', yscrollcommand=vsb.set, xscrollcommand=hsb.set)
        vsb.config(command=self.tree.yview)
        hsb.config(command=self.tree.xview)
        
        # 테이블 헤더 타이틀 설정
        self.tree.heading('timestamp', text='기록/수신 시간 🕒')
        self.tree.heading('header', text='기기 헤더 식별자')
        self.tree.heading('flow', text='순시 유량(m³/h)')
        self.tree.heading('total', text='적산 유량(m³)')
        self.tree.heading('velocity', text='측정 유속(m/s)')
        self.tree.heading('error', text='시스템 에러 코드 상태')
        
        self.tree.column('timestamp', width=220, anchor=tk.CENTER)
        self.tree.column('header', width=120, anchor=tk.CENTER)
        self.tree.column('flow', width=160, anchor=tk.E)
        self.tree.column('total', width=200, anchor=tk.E)
        self.tree.column('velocity', width=160, anchor=tk.E)
        self.tree.column('error', width=180, anchor=tk.CENTER)
        
        self.tree.pack(fill=tk.BOTH, expand=True)

    # ==========================================
    # 3. CALENDAR ENGINE (수동 가동형 달력 위젯 아키텍처)
    # ==========================================
    def draw_calendar(self):
        """프레임 내부에 전월 이동, 금일 비활성화 규칙이 내장된 격자형 달력 드로잉"""
        # 기존 달력 잔여 위젯 파괴 청소
        for widget in self.cal_frame.winfo_children():
            widget.destroy()
            
        # 달력 제어용 헤더바 (<- 년/월 표시 ->)
        cal_header = tb.Frame(self.cal_frame)
        cal_header.pack(fill=tk.X, pady=2)
        
        prev_btn = tb.Button(cal_header, text="◀", bootstyle="link", command=self.prev_month, width=2)
        prev_btn.pack(side=tk.LEFT)
        
        month_title = f"{self.cal_year}년 {self.cal_month:02d}월"
        title_lbl = tb.Label(cal_header, text=month_title, font=("Helvetica", 11, "bold"), anchor=tk.CENTER)
        title_lbl.pack(side=tk.LEFT, expand=True, fill=tk.X)
        
        next_btn = tb.Button(cal_header, text="▶", bootstyle="link", command=self.next_month, width=2)
        next_btn.pack(side=tk.RIGHT)
        
        # 요일 표기판 라인
        days_header = tb.Frame(self.cal_frame)
        days_header.pack(fill=tk.X, pady=2)
        day_names = ["일", "월", "화", "수", "목", "금", "토"]
        for d_name in day_names:
            lbl = tb.Label(days_header, text=d_name, font=("Helvetica", 9), width=4, anchor=tk.CENTER)
            lbl.pack(side=tk.LEFT, expand=True, fill=tk.X)
            
        # 일자 격자 매트릭스 프레임
        grid_frame = tb.Frame(self.cal_frame)
        grid_frame.pack(fill=tk.BOTH, expand=True)
        
        # 1일의 시작 요일 및 해당 월의 총 일수 계산 알고리즘
        try:
            first_day = datetime(self.cal_year, self.cal_month, 1)
            start_weekday = (first_day.weekday() + 1) % 7 # 일요일=0 구조 변환
            if self.cal_month == 12:
                next_first_day = datetime(self.cal_year + 1, 1, 1)
            else:
                next_first_day = datetime(self.cal_year, self.cal_month + 1, 1)
            total_days = (next_first_day - first_day).days
        except Exception:
            return

        # 격자 배치 시작
        row = 0
        col = start_weekday
        
        # 빈칸 채우기
        for _ in range(start_weekday):
            tb.Label(grid_frame, text="", width=4).grid(row=row, column=col)
            
        today_date = datetime.now().date()
        
        for day in range(1, total_days + 1):
            current_loop_date = datetime(self.cal_year, self.cal_month, day).date()
            date_str = f"{self.cal_year}-{self.cal_month:02d}-{day:02d}"
            
            # [규칙 1]: 오늘 날짜는 실시간 전용이므로 클릭 불가능하게 처리(비활성화 스타일)
            if current_loop_date == today_date:
                btn = tb.Button(grid_frame, text=str(day), width=3, bootstyle="secondary-outline", state=tk.DISABLED)
                # 오늘 날짜 밑에 실시간 관제 상태 표시 레이블 오버레이 효과
                btn.grid(row=row, column=col, padx=2, pady=2)
            # 미래 날짜 역시 비활성화
            elif current_loop_date > today_date:
                btn = tb.Button(grid_frame, text=str(day), width=3, bootstyle="dark", state=tk.DISABLED)
                btn.grid(row=row, column=col, padx=2, pady=2)
            # [규칙 2]: 어제 날짜를 포함한 과거 로그 데이터가 저장된 날짜는 선명히 활성화 및 클릭 연동
            else:
                # 과거 데이터 파일 존재 여부 미리 체크하여 시각 효과 차별화
                target_file = os.path.join(SECURE_DATA_DIR, f"flow_log_{date_str}.csv")
                b_style = "info" if os.path.exists(target_file) else "secondary"
                
                btn = tb.Button(
                    grid_frame, text=str(day), width=3, bootstyle=b_style,
                    command=lambda d=date_str: self.load_history_data(d)
                )
                btn.grid(row=row, column=col, padx=2, pady=2)
                
            col += 1
            if col > 6:
                col = 0
                row += 1

    def prev_month(self):
        """달력 이전 달 이동 화살표 액션"""
        self.cal_month -= 1
        if self.cal_month < 1:
            self.cal_month = 12
            self.cal_year -= 1
        self.draw_calendar()

    def next_month(self):
        """달력 다음 달 이동 화살표 액션"""
        self.cal_month += 1
        if self.cal_month > 12:
            self.cal_month = 1
            self.cal_year += 1
        self.draw_calendar()

    # ==========================================
    # 4. HISTORY DATA CORE ENGINE (과거 로그 데이터 수집 및 분석 연동)
    # ==========================================
    def load_history_data(self, date_string):
        """과거 특정 일자의 CSV 파일을 로드하여 24시간 대용량 차트 표출 및 스크롤 바인딩"""
        target_file = os.path.join(SECURE_DATA_DIR, f"flow_log_{date_string}.csv")
        
        if not os.path.exists(target_file):
            Messagebox.show_warning(f"[{date_string}] 해당 날짜의 유량 측정 로그 파일이 존재하지 않습니다.", "데이터 누락 공지")
            return
            
        # 과거 데이터 로드 모드 전환
        self.is_history_mode = True
        self.history_data_list.clear()
        
        # 기존 테이블 청소
        for row in self.tree.get_children():
            self.tree.delete(row)
            
        # CSV 스트리밍 리딩 파싱
        try:
            with open(target_file, mode='r', encoding='utf-8') as f:
                reader = csv.reader(f)
                header_row = next(reader, None) # 헤더 스킵
                
                for row in reader:
                    if len(row) >= 6:
                        data_node = {
                            'timestamp': row[0],
                            'header': row[1],
                            'flow': float(row[2]),
                            'total': float(row[3]),
                            'velocity': float(row[4]),
                            'error': row[5]
                        }
                        self.history_data_list.append(data_node)
        except Exception as e:
            Messagebox.show_error(f"로그 파일 판독 실패: {e}", "파일 시스템 오류")
            return

        if not self.history_data_list:
            Messagebox.show_info("선택된 날짜 파일 내 레코드가 비어 있습니다.", "데이터 공백")
            return

        # 대용량 리스트 렌더링 가속 (하단 테이블에 순차 기록)
        # 처리 속도를 위해 상위 2000개만 UI 테이블에 선표출하되, 정밀 상하좌우 스크롤바 바인딩
        for node in reversed(self.history_data_list):
            self.tree.insert('', tk.END, values=(
                node['timestamp'], node['header'], 
                f"{node['flow']:.3f}", f"{node['total']:.3f}", 
                f"{node['velocity']:.2f}", node['error']
            ))

        # 우측 상단 그래픽 차트에 24시간 풀 타임라인 데이터 빌드 및 전역 플로팅
        self.render_history_chart(date_string)
        self.status_lbl.config(text=f"📊과거 이력 조회 중: {date_string} (총 {len(self.history_data_list)}건)", bootstyle="info")

    def render_history_chart(self, date_title):
        """24시간 대용량 이력 그래프를 그리고 줌/팬 탐색 환경 제공"""
        self.ax.clear()
        
        # 시계열 추출
        times = [datetime.strptime(node['timestamp'], '%Y-%m-%d %H:%M:%S') for node in self.history_data_list]
        flows = [node['flow'] for node in self.history_data_list]
        
        # 선명한 보라색/인디고 스타일 이력 파형 드로잉 효과
        self.ax.plot(times, flows, color='#3498db', linewidth=1.5, label=f'{date_title} Flow History')
        self.ax.set_title(f"📈 {date_title} 일일 유량 분석 트렌드 (차트 내 마우스 클릭 시 아래 데이터 연동 이동)")
        self.ax.grid(True, color='#2c3e50', linestyle='--')
        self.ax.legend(loc='upper right')
        
        # X축 날짜 시간 포맷 설정 최적화
        self.fig.autofmt_xdate()
        
        # 플롯 리프레시 업데이트
        self.canvas.draw()

    # ==========================================
    # 5. CHART INTERACTION (특정 시간대 매칭 검색 기술)
    # ==========================================
    def on_chart_click(self, event):
        """과거 이력 그래프에서 임의의 파형 시점을 마우스로 클릭하면 해당 시간에 대응되는 리스트 행으로 자동 스크롤 추적"""
        if not self.is_history_mode or event.xdata is None:
            return
            
        # Matplotlib 내부의 부동소수점 날짜 포맷을 파이썬 datetime 데이터로 복원
        click_dt = plt.matplotlib.dates.num2date(event.xdata).replace(tzinfo=None)
        
        # 이력 데이터 배열에서 마우스로 클릭한 시간과 가장 가까운 노드 색인 검색
        closest_node = min(self.history_data_list, key=lambda x: abs(datetime.strptime(x['timestamp'], '%Y-%m-%d %H:%M:%S') - click_dt))
        target_timestamp = closest_node['timestamp']
        
        # 하단 데이터 테이블 트리뷰 내에서 동일한 타임스탬프를 가진 데이터 행(Row) 추적
        for item_id in self.tree.get_children():
            row_values = self.tree.item(item_id)['values']
            if row_values and row_values[0] == target_timestamp:
                # 해당 행을 선택 상태로 만들고 시야 정중앙으로 강제 상하 스크롤 이동
                self.tree.selection_set(item_id)
                self.tree.see(item_id)
                self.status_lbl.config(text=f"🎯 선택 시점 추적 완료: {target_timestamp}", bootstyle="info")
                break

    # ==========================================
    # 6. REAL-TIME CORE ENGINE (기존 실시간 소스코드 동기화 유지)
    # ==========================================
    def toggle_test_mode(self):
        """[테스트] 시뮬레이션 버튼 토글 액션 처리"""
        self.test_mode = not self.test_mode
        if self.test_mode:
            self.is_history_mode = False # 실시간 화면 복귀
            self.test_btn.config(text="🔴 REC (시뮬레이션 중)", bootstyle="danger")
            self.status_lbl.config(text="가상 시뮬레이션 데이터 발생 및 실시간 CSV 기록 중...", bootstyle="danger")
            
            # 차트 초기화 (실시간 파형 라인 복구)
            self.ax.clear()
            self.line, = self.ax.plot(self.flow_history, color='#00bc8c', linewidth=2.5, label='Flow Rate (m³/h)')
            self.ax.set_ylim(0, 50)
            self.ax.grid(True, color='#2c3e50', linestyle='--')
            self.ax.legend(loc='upper left')
            
            self.sim_thread = threading.Thread(target=self.test_data_generator, daemon=True)
            self.sim_thread.start()
        else:
            self.test_btn.config(text="⚙️ 시뮬레이션 TEST 시작", bootstyle="outline-warning")
            self.status_lbl.config(text=f"연결 대기중 ({SERIAL_PORT})", bootstyle="warning")

    def start_backend_services(self):
        """실시간 시리얼 및 애니메이션 바인딩 엔진"""
        self.serial_thread = threading.Thread(target=self.serial_receiver_loop, daemon=True)
        self.serial_thread.start()
        
        self.ani = animation.FuncAnimation(self.fig, self.update_chart, interval=1000, blit=False)
        self.root.protocol("WM_DELETE_WINDOW", self.on_closing)

    def write_to_csv(self, timestamp, header, flow, total, velocity, error):
        """1일 단위 날짜 매커니즘 분할 저장"""
        current_date = datetime.now().strftime('%Y-%m-%d')
        file_path = os.path.join(SECURE_DATA_DIR, f"flow_log_{current_date}.csv")
        file_exists = os.path.exists(file_path)
        
        try:
            with open(file_path, mode='a', newline='', encoding='utf-8') as f:
                writer = csv.writer(f)
                if not file_exists:
                    writer.writerow(['Timestamp', 'Header', 'Flow_Rate(m3/h)', 'Total_Flow(m3)', 'Velocity(m/s)', 'Error_Code'])
                writer.writerow([timestamp, header, flow, total, velocity, error])
                
            # 신규 파일이 0시에 자동 생성될 때 실시간 달력 새로고침 유도
            if not file_exists:
                self.root.after(0, self.draw_calendar)
        except Exception as e:
            print(f"[-] 파일 기록 누락 오류: {e}")

    def test_data_generator(self):
        """실시간 자연 데이터 난수 스트리밍 발생 엔진 (1초 단위)"""
        base_flow = 22.8
        while self.running and self.test_mode:
            timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
            header = "SIM_X100L"
            
            base_flow = max(4.0, min(48.0, base_flow + random.uniform(-1.5, 1.5)))
            self.current_flow = base_flow
            self.current_total += (self.current_flow / 3600.0)
            self.current_velocity = (self.current_flow / 12.8) + random.uniform(-0.02, 0.02)
            self.error_code = "1100000000" if random.random() > 0.99 else "0000000000"
            
            self.flow_history.pop(0)
            self.flow_history.append(self.current_flow)
            
            self.root.after(0, self.update_ui_elements, timestamp, header)
            self.write_to_csv(timestamp, header, round(self.current_flow, 3), round(self.current_total, 3), round(self.current_velocity, 2), self.error_code)
            
            time.sleep(1.0)

    def serial_receiver_loop(self):
        """실제 하드웨어 포트 실시간 리시버 세트"""
        while self.running:
            if self.test_mode or self.is_history_mode:
                time.sleep(1)
                continue
            try:
                ser = serial.Serial(
                    port=SERIAL_PORT, baudrate=BAUD_RATE,
                    bytesize=serial.EIGHTBITS, parity=serial.PARITY_NONE,
                    stopbits=serial.STOPBITS_ONE, timeout=1.5
                )
                self.root.after(0, lambda: self.status_lbl.config(text="통신 연결 활성화 (OK)", bootstyle="success"))
                
                while self.running and not self.test_mode and not self.is_history_mode:
                    if ser.in_waiting > 0:
                        raw_line = ser.readline()
                        decoded_line = raw_line.decode('ascii', errors='ignore').strip()
                        
                        if not decoded_line:
                            continue
                        
                        parts = [p.strip() for p in decoded_line.split(',')]
                        if len(parts) >= 4:
                            timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
                            header = parts[0]
                            flow_str = parts[1].replace('+', '').strip()
                            total_str = parts[2].replace('+', '').strip()
                            error_str = parts[3]
                            velocity_str = parts[4] if len(parts) > 4 else "0.00"
                            
                            try:
                                self.current_flow = float(flow_str)
                                self.current_total = float(total_str)
                                self.current_velocity = float(velocity_str)
                                self.error_code = error_str
                                
                                self.flow_history.pop(0)
                                self.flow_history.append(self.current_flow)
                                
                                self.root.after(0, self.update_ui_elements, timestamp, header)
                                self.write_to_csv(timestamp, header, self.current_flow, self.current_total, self.current_velocity, self.error_code)
                            except ValueError:
                                pass
                    time.sleep(0.05)
                ser.close()
            except serial.SerialException:
                if not self.test_mode and not self.is_history_mode:
                    self.root.after(0, lambda: self.status_lbl.config(text="연결 차단됨 - 포트 확인", bootstyle="danger"))
                time.sleep(3)
            except Exception:
                time.sleep(3)

    def update_ui_elements(self, timestamp, header):
        """실시간 GUI 컴포넌트 데이터 텍스트 밀어주기"""
        if self.is_history_mode:
            return
        self.flow_val_lbl.config(text=f"{self.current_flow:,.3f} m³/h")
        self.total_val_lbl.config(text=f"{self.current_total:,.3f} m³")
        self.vel_val_lbl.config(text=f"유속: {self.current_velocity:.2f} m/s")
        self.err_val_lbl.config(text=f"Error: {self.error_code}")
        
        self.tree.insert('', 0, values=(timestamp, header, f"{self.current_flow:.3f}", f"{self.current_total:.3f}", f"{self.current_velocity:.2f}", self.error_code))
        if len(self.tree.get_children()) > 100:
            self.tree.delete(self.tree.get_children()[-1])

    def update_chart(self, frame):
        """실시간 스트리밍 시 축 줌 자동 연동 제어"""
        if self.is_history_mode:
            return self.line,
            
        self.line.set_ydata(self.flow_history)
        max_val = max(self.flow_history)
        min_val = min(self.flow_history)
        margin = max(5.0, (max_val - min_val) * 0.15)
        
        self.ax.set_ylim(max(0, min_val - margin), max_val + margin)
        self.canvas.draw_idle()
        return self.line,

    def on_closing(self):
        self.running = False
        self.root.destroy()

if __name__ == "__main__":
    root = tb.Window(themename="superhero")
    app = AdvancedFlowDashboard(root)
    root.mainloop()