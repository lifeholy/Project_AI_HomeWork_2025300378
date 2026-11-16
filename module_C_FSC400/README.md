# 모듈 C: 산업용 유량계(FSC400) 실시간 관제 — Rust 버전

- **과목:** 인공지능특론세미나 (01반) 기말과제
- **학교:** 한양사이버대학교 일반대학원 기계IT융합공학과
- **학번 / 이름:** 2025300378 / 이민호

모듈 A(Python, `basecode/`)와 같은 기능을 Rust로 다시 작성한 프로그램입니다. 화면 구성, 데이터 흐름, CSV 형식, 설정 파일을 모듈 A와 맞췄습니다.

## 상태

- **소스:** 작성 완료 (`src/`)
- **문법 검사:** `rustfmt`로 모든 파일 통과
- **컴파일·실행 파일:** **아직 빌드되지 않았습니다.** 이 PC에 Windows SDK(`kernel32.lib` 등)가 없어 Rust MSVC 링커가 실패합니다. 아래 "빌드 방법"의 SDK 설치 후 빌드하면 `배포용/fsc400_dashboard.exe`가 생성됩니다.
- **단위 테스트:** 프로토콜(CRC, 요청 프레임, CDAB 디코딩, 응답 파싱), ASCII 라인 파서, CSV 기록·읽기 테스트를 `src/`에 작성했습니다. 빌드 후 `cargo test`로 실행합니다.

## 구성

| 경로 | 내용 |
|---|---|
| `src/main.rs` | 진입점 (창 설정) |
| `src/app.rs` | 관제 GUI (egui): 수치 카드, 달력, 차트, 목록, 과거 기록 |
| `src/sources.rs` | 수신 스레드: MODBUS-RTU / ASCII 라인 / 시뮬레이션, 재연결 |
| `src/protocol.rs` | MODBUS-RTU 프레이밍, CRC-16, CDAB 디코딩 (매뉴얼 부록 A) |
| `src/storage.rs` | 일별 CSV 기록·읽기, `settings.json` |
| `배포용/` | 빌드 후 `fsc400_dashboard.exe`, `사용설명.txt` |
| `basecode/` | 모듈 A의 Python 원본 (`src/fsc400_dashboard.py`, 배포용 Python exe, `samplecode/`) |
| `docs/` | 프로토콜 정리 문서, 매뉴얼 PDF |

## 사용법 (배포용 exe 기준)

- 화면 구성, 연결 설정, 과거 기록 조회, 생성 파일은 모듈 A 사용설명과 같습니다. 자세한 내용은 `배포용/사용설명.txt`를 보세요.
- 데이터 소스: `MODBUS-RTU`(기본), `ASCII 라인`, `시뮬레이션`
- 생성 파일 (exe와 같은 폴더): `data\flow_log_YYYY-MM-DD.csv`, `settings.json`
- 폰트: Windows의 `맑은 고딕`(`C:\Windows\Fonts\malgun.ttf`)을 읽어 한글을 표시합니다.

## 빌드 방법

1. Windows SDK 설치 (한 번만). 관리자 권한 PowerShell 또는 `!` 명령으로 실행합니다.
   ```
   winget install --id Microsoft.WindowsSDK.10.0.22621 --exact --source winget
   ```
   또는 Visual Studio Installer에서 "Windows 10/11 SDK" 구성 요소를 추가합니다.
2. 빌드 (최신 stable Rust, MSVC 툴체인):
   ```
   cd module_C_FSC400
   cargo test
   cargo build --release
   copy target\release\fsc400_dashboard.exe 배포용\
   ```
3. 배포용 폴더에 `사용설명.txt`를 함께 둡니다.

## 모듈 A와의 차이

- 언어와 GUI 라이브러리: Python(tkinter/ttkbootstrap) → Rust(egui/eframe)
- 시리얼: pyserial → serialport
- 차트: matplotlib → egui_plot (확대·이동 지원, 설정된 축 포맷)
- 입력 검증과 오류 처리는 같지만, 오류 문구는 일부 다를 수 있습니다.

## 알려진 제한

- MODBUS 에러 코드는 레지스터 맵에 없어 `N/A`로 표시됩니다.
- ASCII 라인 형식과 실제 장비 출력의 일치 여부는 실장비 확인이 필요합니다.
- 측정 시각은 PC 수신 시각입니다.
