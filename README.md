# Project_AI_HomeWork1

인공지능특론세미나 (01반) 기말과제

- **학교:** 한양사이버대학교 일반대학원 기계IT융합공학과
- **학번 / 이름:** 2025300378 / 이민호

Windows 현장 장비와 서버를 감시하는 관제 프로그램 네 개를 담고 있습니다. A·B는 Python으로, C·D는 Rust로 같은 기능을 구현했습니다. 배포는 Python 또는 Rust로 만든 단일 exe 파일로 합니다.

## 구성

| 모듈 | 대상 | 언어 / GUI | 배포 실행 파일 | 상태 | 상세 문서 |
|---|---|---|---|---|---|
| A. 유량계 관제 (Python) | 설치형 초음파 유량계 FSC400 | Python / ttkbootstrap | `module_A_FSC400/배포용/fsc400_dashboard.exe` | 빌드·실행 확인 | [module_A_FSC400/README.md](module_A_FSC400/README.md) |
| B. 서버 관제 (Python) | Windows 서버 AIRBOOK-FAS | Python / PyQt6 | `module_B_AIRBOOK-FAS/배포용/checkserver.exe` | 빌드·실행 확인 | [module_B_AIRBOOK-FAS/README.md](module_B_AIRBOOK-FAS/README.md) |
| C. 유량계 관제 (Rust) | 설치형 초음파 유량계 FSC400 | Rust / egui | `module_C_FSC400/배포용/fsc400_dashboard.exe` (빌드 대기) | 소스 작성, 문법 검사 통과 | [module_C_FSC400/README.md](module_C_FSC400/README.md) |
| D. 서버 관제 (Rust) | Windows 서버 AIRBOOK-FAS | Rust / egui | `module_D_AIRBOOK-FAS/배포용/checkserver.exe` (빌드 대기) | 소스 작성, 문법 검사 통과 | [module_D_AIRBOOK-FAS/README.md](module_D_AIRBOOK-FAS/README.md) |

- A·B는 기존 Python 버전이며, C·D는 같은 기능을 Rust로 다시 작성한 버전입니다.
- C·D 폴더에는 Python 원본(A·B와 같은 코드)이 `basecode/` 하위에 보관되어 있습니다.

## 배포용 실행 파일

### A. fsc400_dashboard.exe (유량계 관제, Python)

- **포함 파일:** `fsc400_dashboard.exe`(약 66 MB), `사용설명.txt`
- **실행:** 더블클릭하면 창이 계속 떠 있으며 데이터를 수신합니다.
- **데이터 소스:** `MODBUS-RTU`(매뉴얼 부록 A, 기본값), `ASCII 라인`(samplecode 방식), `시뮬레이션`
- **화면:** 수치 카드, 실시간 차트, 최신순 목록
- **과거 기록:** 달력에서 날짜를 고르면 하루치 차트와 목록을 보고, 차트 클릭으로 목록 행을 찾습니다.
- **생성 파일** (exe와 같은 폴더): `data\flow_log_YYYY-MM-DD.csv`, `settings.json`

### B. checkserver.exe (서버 관제, Python)

- **포함 파일:** `checkserver.exe`(약 35 MB), `userset.json`(exe와 같은 폴더에 있어야 함), `사용설명.txt`
- **실행:** 더블클릭. 일부 기능(서비스 조회, 프로세스 상세)은 관리자 권한이 필요할 수 있습니다.
- **기능:** CPU·RAM·DISK·프로세스 게이지, 임계치 경보와 경고음, 프로세스 탭, 점검 배치 실행, 결과 조회, 로그, 설정
- **생성 파일:** `C:\checkPMI\` 아래 점검 배치, 실행 결과(`logs\<시각>\`), 누적 로그(`logs\system_accumulated.log`)

### C. fsc400_dashboard.exe (유량계 관제, Rust) — 빌드 대기

- A와 같은 기능과 화면입니다. 설정 파일과 CSV 형식도 같습니다.
- 빌드 전에 Windows SDK 설치가 필요합니다. 자세한 방법은 `module_C_FSC400/README.md`를 보세요.

### D. checkserver.exe (서버 관제, Rust) — 빌드 대기

- B와 같은 기능과 탭 구성입니다. `userset.json` 스키마도 같습니다.
- 빌드 전에 Windows SDK 설치가 필요합니다. 자세한 방법은 `module_D_AIRBOOK-FAS/README.md`를 보세요.

## 개발 환경 실행

```
# Python 버전 (A, B)
pip install ttkbootstrap matplotlib pyserial          # A
pip install PyQt6 psutil                              # B
python module_A_FSC400/src/fsc400_dashboard.py
python module_B_AIRBOOK-FAS/src/CheckServer.py

# Rust 버전 (C, D) — 최신 stable Rust, Windows SDK 필요
cd module_C_FSC400 && cargo run --release
cd module_D_AIRBOOK-FAS && cargo run --release
```

## exe 빌드

### Python (A, B)

```
pyinstaller --noconfirm --clean --onefile --windowed --name fsc400_dashboard ^
  --distpath 배포용 --workpath %TEMP%\pyi_a --specpath %TEMP%\pyi_a src\fsc400_dashboard.py

pyinstaller --noconfirm --clean --onefile --windowed --name checkserver ^
  --distpath 배포용 --workpath %TEMP%\pyi_b --specpath %TEMP%\pyi_b src\CheckServer.py
copy src\userset.json 배포용\
```

### Rust (C, D)

```
cargo build --release
copy target\release\fsc400_dashboard.exe 배포용\    # C
copy target\release\checkserver.exe 배포용\          # D
```

## 폴더 구조

```
Project_AI_HomeWork1/
├── README.md
├── module_A_FSC400/               # 유량계 관제 (Python)
│   ├── README.md
│   ├── src/fsc400_dashboard.py
│   ├── docs/protocol_FSC400.md
│   ├── samplecode/
│   ├── 배포용/                     # fsc400_dashboard.exe, 사용설명.txt
│   └── _archive_cli/              # 이전 CLI 수집기 (보관용)
├── module_B_AIRBOOK-FAS/          # 서버 관제 (Python)
│   ├── README.md
│   ├── src/CheckServer.py, userset.json
│   ├── samplecode/
│   ├── 배포용/                     # checkserver.exe, userset.json, 사용설명.txt
│   └── _archive_cli/              # 이전 CLI 진단기 (보관용)
├── module_C_FSC400/               # 유량계 관제 (Rust)
│   ├── README.md, Cargo.toml, .gitignore
│   ├── src/                       # main, app, sources, protocol, storage
│   ├── docs/                      # 프로토콜 정리, 매뉴얼 PDF
│   ├── 배포용/                     # 빌드 후 fsc400_dashboard.exe, 사용설명.txt
│   └── basecode/                  # 모듈 A Python 원본 (src, samplecode, 배포용)
└── module_D_AIRBOOK-FAS/          # 서버 관제 (Rust)
    ├── README.md, Cargo.toml, .gitignore
    ├── src/                       # main, app, monitor, config, batch, logs, bat/
    ├── 배포용/                     # 빌드 후 checkserver.exe, userset.json, 사용설명.txt
    └── basecode/                  # 모듈 B Python 원본 (src, samplecode, 배포용)
```

## 검증 현황

- **A (Python):** MODBUS 프레이밍과 워드 순서 디코딩을 매뉴얼 예제로 확인했습니다. 시뮬레이션 수신, 목록·차트 갱신, CSV 기록, 과거 기록 조회를 자동 스크립트와 화면 캡처로 확인했습니다. exe 실행도 확인했습니다. 실장비 연결은 시험하지 못했습니다.
- **B (Python):** 소스 컴파일, 메인 창 실행(오프스크린), exe 실행을 확인했습니다. 실서버 점검 배치, 경고음, 관리자 권한 기능은 확인하지 못했습니다.
- **C, D (Rust):** 소스를 작성하고 `rustfmt`로 문법 검사를 통과했습니다. **컴파일과 테스트는 아직 실행하지 못했습니다.** 이 PC에 Windows SDK(`kernel32.lib`)가 없어 링커가 실패하기 때문입니다. SDK 설치 후 `cargo test`와 `cargo build --release`로 확인해야 합니다. 설치는 관리자 권한이 필요하므로 사용자가 직접 실행해야 합니다.

## 참고

- **보안:** B와 D의 내장 점검 배치와 `userset.json`에는 내부 IP 주소가 들어 있습니다. 과제 보안 기준상 공개 저장소에 올리기 전에 마스킹해야 합니다.
- **저장소 정리:** 빌드 산출물(`target/`, `__pycache__/`), 실행 중 생성된 `data/`와 `settings.json`은 `.gitignore`로 제외하는 것이 좋습니다.
- 이전 CLI 버전은 `_archive_cli/`에 보관되어 있으며 사용하지 않습니다.
