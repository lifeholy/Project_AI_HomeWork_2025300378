#!/usr/bin/env python3
import os
import sys
import csv
import random
from datetime import datetime, timedelta

# 대시보드와 동일한 저장 경로 설정 (Windows 환경일 경우 상대경로, 리눅스일 경우 절대경로)
SECURE_DATA_DIR = './secure_data' if sys.platform == 'win32' else '/var/www/secure/data'

if not os.path.exists(SECURE_DATA_DIR):
    os.makedirs(SECURE_DATA_DIR)

def generate_daily_static_csv(target_date_str):
    """
    원하는 날짜(YYYY-MM-DD)를 입력받아 86,400초 동안의 
    연속적이고 자연스러운 가상 유량 데이터를 생성하여 CSV로 저장합니다.
    """
    # 1. 입력 날짜 유효성 검증 및 시작/종료 시점 계산
    try:
        start_dt = datetime.strptime(target_date_str, "%Y-%m-%d")
        # 해당 날짜의 00:00:00 설정
        start_dt = start_dt.replace(hour=0, minute=0, second=0)
    except ValueError:
        print("[-] 날짜 형식이 올바르지 않습니다. YYYY-MM-DD 형식으로 입력해주세요. (예: 2026-06-03)")
        return

    file_name = f"flow_log_{target_date_str}.csv"
    file_path = os.path.join(SECURE_DATA_DIR, file_name)
    
    print(f"[+] [{target_date_str}] 하루치(86,400초) 가상 데이터 생성을 시작합니다...")
    print(f"[+] 저장 경로: {file_path}")

    # 2. 물리적 가상 변수 초기화
    base_flow = random.uniform(15.0, 30.0)  # 시작 순시 유량 기준값 (m3/h)
    total_flow = random.uniform(100.0, 500.0) # 시작 적산 유량 초기값 (m3)
    error_code = "0000000000"
    header = "SIM_X100L"

    # 하루는 총 86,400초
    total_seconds = 24 * 60 * 60  

    # 3. CSV 파일 쓰기 프로세스 시작
    try:
        with open(file_path, mode='w', newline='', encoding='utf-8') as f:
            writer = csv.csv.writer(f) if hasattr(csv, 'csv') else csv.writer(f)
            
            # 대시보드 호환 필드 헤더 작성
            writer.writerow(['Timestamp', 'Header', 'Flow_Rate(m3/h)', 'Total_Flow(m3)', 'Velocity(m/s)', 'Error_Code'])
            
            # 86,400번의 루프를 돌며 1초 단위 데이터 연산
            for i in range(total_seconds):
                # 1초씩 증가하는 타임스탬프 생성
                current_dt = start_dt + timedelta(seconds=i)
                timestamp_str = current_dt.strftime('%Y-%m-%d %H:%M:%S')
                
                # 시계열 트렌드가 급격히 튀지 않도록 마진이 제한된 랜덤 워크 알고리즘 적용
                # 낮과 밤의 유량 변화율을 묘사하기 위해 사인파(Sine Wave) 주기성 소량 추가
                time_factor = random.math.sin(2 * random.math.pi * i / total_seconds) if hasattr(random, 'math') else 0
                flow_fluctuation = random.uniform(-0.4, 0.4) + (time_factor * 0.05)
                
                base_flow = max(2.0, min(60.0, base_flow + flow_fluctuation))
                
                # 1초당 누적 유량 반영 (m3/h 이므로 3600초로 나누어 누적)
                total_flow += (base_flow / 3600.0)
                
                # 배관 직경 대입 물리 공식 기반 유속 가상 산출
                velocity = (base_flow / 12.8) + random.uniform(-0.01, 0.01)
                
                # 데이터 라인 작성
                writer.writerow([
                    timestamp_str,
                    header,
                    f"{base_flow:.3f}",
                    f"{total_flow:.3f}",
                    f"{velocity:.2f}",
                    error_code
                ])
                
                # 대량 생성 중 진행률 표시 (10% 단위)
                if (i + 1) % (total_seconds // 10) == 0:
                    progress = ((i + 1) / total_seconds) * 100
                    print(f" -> 데이터 생성 진행률: {int(progress)}% 완료 ({timestamp_str})")
                    
        print(f"[▲] 성공: [{file_name}] 파일 생성이 완료되었습니다! 총 {total_seconds:,}행 기록됨.")
        
    except Exception as e:
        print(f"[-] 파일 생성 중 시스템 오류 발생: {e}")

if __name__ == "__main__":
    # 사용자가 스크립트 실행 시 직접 날짜를 인자로 넘겨줄 수 있게 처리
    # 예: python generate_fake_day.py 2026-06-03
    if len(sys.argv) > 1:
        user_date = sys.argv[1]
    else:
        # 별도 인자가 없을 경우 콘솔에서 직접 물어보기
        user_date = input("생성하고자 하는 가상 과거 날짜를 입력하세요 (YYYY-MM-DD): ").strip()
        
    generate_daily_static_csv(user_date)