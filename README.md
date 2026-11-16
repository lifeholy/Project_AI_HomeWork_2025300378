# Project_AI_HomeWork1

인공지능특론세미나 (01반) 기말과제

- **학교:** 한양사이버대학교 일반대학원 기계IT융합공학과
- **학번 / 이름:** 2025300378 / 이민호

Windows 현장 장비와 서버를 감시하는 두 개의 관제 프로그램을 담고 있습니다. 각 프로그램은 Python 없이 실행할 수 있는 단일 exe 파일로 배포합니다.

## 구성

| 모듈 | 대상 | 배포 실행 파일 | 상세 문서 |
|---|---|---|---|
| A. 유량계 관제 | 설치형 초음파 유량계 FSC400 | `module_A_FSC400/배포용/fsc400_dashboard.exe` | [module_A_FSC400/README.md](module_A_FSC400/README.md) |
| B. 서버 관제 | Windows 서버 AIRBOOK-FAS | `module_B_AIRBOOK-FAS/배포용/checkserver.exe` | [module_B_AIRBOOK-FAS/README.md](module_B_AIRBOOK-FAS/README.md) |

## 배포용 실행 파일

### A. fsc400_dashboard.exe (유량계 관제)

- **포함 파일**
  - `fsc400_dashboard.exe` (약 66 MB, 단일 파일)
  - `사용설명.txt`
- **실행:** 더블클릭. 창이 계속 떠 있으며 데이터를 수신합니다.
- **데이터 소스 (화면 상단에서 선택)**
  - `MODBUS-RTU`: FSC400 매뉴얼 부록 A 방식. 기본값. 1초마다 레지스터를 읽습니다.
  - `ASCII 라인`: "헤더,유량,적산,에러,유속" 형식을 한 줄씩 받는 장비용
  - `시뮬레이션`: 장비 없이 가상 데이터 생성
- **화면:** 순시 유량, 적산 유량, 유속, 에러 코드 카드, 실시간 차트, 최신순 목록
- **과거 기록:** 왼쪽 달력에서 파란색 날짜를 누르면 하루치 차트와 목록을 봅니다. 차트를 클릭하면 목록의 해당 시점 행이 선택됩니다.
- **생성되는 파일** (exe와 같은 폴더)
  - `data\flow_log_YYYY-MM-DD.csv`: 일별 기록 (1초 주기, 열: Timestamp, Header, Flow_Rate(m3/h), Total_Flow(m3), Velocity(m/s), Error_Code)
  - `settings.json`: 마지막 연결 설정
- **주의:** 에러 코드는 MODBUS 레지스터 맵에 없어 `N/A`로 표시됩니다. ASCII 라인 방식은 실제 장비 출력 형식과 일치하는지 확인이 필요합니다.

### B. checkserver.exe (서버 관제)

- **포함 파일**
  - `checkserver.exe` (약 35 MB, 단일 파일)
  - `userset.json` (설정 파일, **exe와 같은 폴더에 있어야 함**)
  - `사용설명.txt`
- **실행:** `checkserver.exe` 더블클릭. 일부 기능(서비스 조회, 프로세스 상세 정보)은 관리자 권한이 필요할 수 있습니다.
- **탭 구성**
  - 대시보드: CPU, RAM, DISK, 프로세스 수 게이지. 임계치 초과 시 화면이 붉게 바뀌고 경고음이 울립니다.
  - 프로세스: 실시간 목록, 감시 대상 강조, 더블클릭 시 실행 파일 경로와 명령줄 표시
  - 점검실행: 일일 FAS / EAI 점검 배치 실행
  - 결과조회: 날짜별 점검 결과(TXT, CSV) 열람
  - 로그: 누적 로그 열람과 검색
  - 설정: 감시 프로세스, 임계치, 감시 드라이브, 시간 동기화 명령, 배치 경로
- **생성되는 파일** (첫 실행 시 자동 생성)
  - `C:\checkPMI\F100-2-001.bat`, `C:\checkPMI\F100-1-001-DETAIL.bat`: 내장 점검 배치 (없을 때만 생성)
  - `C:\checkPMI\logs\<yyyymmdd_hhmmss>\check_result.txt / .csv`: 점검 결과
  - `C:\checkPMI\logs\system_accumulated.log`: 누적 로그
- **주의:** 설정 탭의 배치 경로와 감시 대상은 `userset.json`에 저장됩니다. 점검 배치의 대상 주소는 현장 환경에 맞게 확인이 필요합니다.

## 개발 환경 실행

Python 3.12 기준입니다.

```
pip install ttkbootstrap matplotlib pyserial          # 모듈 A
pip install PyQt6 psutil                              # 모듈 B

python module_A_FSC400/src/fsc400_dashboard.py
python module_B_AIRBOOK-FAS/src/CheckServer.py
```

## exe 빌드 방법

PyInstaller로 단일 파일을 만듭니다. 중간 산출물은 저장소 밖(`%TEMP%`)에 두고, 결과만 `배포용` 폴더에 씁니다.

```
# 모듈 A
cd module_A_FSC400
pyinstaller --noconfirm --clean --onefile --windowed --name fsc400_dashboard ^
  --distpath 배포용 --workpath %TEMP%\pyi_a --specpath %TEMP%\pyi_a src\fsc400_dashboard.py

# 모듈 B (빌드 후 userset.json을 exe 옆에 복사)
cd module_B_AIRBOOK-FAS
pyinstaller --noconfirm --clean --onefile --windowed --name checkserver ^
  --distpath 배포용 --workpath %TEMP%\pyi_b --specpath %TEMP%\pyi_b src\CheckServer.py
copy src\userset.json 배포용\
```

## 폴더 구조

```
Project_AI_HomeWork1/
├── README.md
├── module_A_FSC400/            # 유량계 관제 (FSC400)
│   ├── README.md
│   ├── src/fsc400_dashboard.py # GUI 본체
│   ├── docs/protocol_FSC400.md # MODBUS 레지스터 맵 정리
│   ├── samplecode/             # 참고 원본
│   ├── 배포용/                  # fsc400_dashboard.exe, 사용설명.txt
│   └── _archive_cli/           # 이전 CLI 수집기 (보관용)
└── module_B_AIRBOOK-FAS/       # 서버 관제 (AIRBOOK-FAS)
    ├── README.md
    ├── src/CheckServer.py      # GUI 본체
    ├── src/userset.json        # 설정 파일
    ├── samplecode/             # 참고 원본
    ├── 배포용/                  # checkserver.exe, userset.json, 사용설명.txt
    └── _archive_cli/           # 이전 CLI 진단기 (보관용)
```

## 검증 현황

- **모듈 A:** MODBUS 프레이밍(CRC, 응답 파싱, 워드 순서 디코딩)은 매뉴얼 예제 값으로 확인했습니다. 시뮬레이션 수신, 목록·차트 갱신, 일별 CSV 기록, 과거 기록 조회, 차트 클릭 연동은 자동 스크립트와 화면 캡처로 확인했습니다. exe 실행도 확인했습니다. 실제 FSC400 장비 연결은 아직 시험하지 못했습니다.
- **모듈 B:** 소스 컴파일과 메인 창 실행을 오프스크린으로 확인했고, exe가 충돌 없이 실행됩니다. 실제 점검 배치 실행, 경고음, 관리자 권한 기능은 실서버 없이 확인하지 못했습니다.

## 참고

- 장비 명세서(FSC400 매뉴얼)와 과제 업무지시서는 저장소에 포함되어 있지 않습니다. 매뉴얼 PDF는 `module_A_FSC400` 폴더에 있습니다.
- 이전 CLI 버전(`_archive_cli/`)은 참고용으로만 남겨 두었습니다.
