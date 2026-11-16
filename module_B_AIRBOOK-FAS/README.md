# 모듈 B: Windows 서버(AIRBOOK-FAS) 점검·실시간 감시 GUI

- **과목:** 인공지능특론세미나 (01반) 기말과제
- **학교:** 한양사이버대학교 일반대학원 기계IT융합공학과
- **학번 / 이름:** 2025300378 / 이민호

## 구성
- `src/CheckServer.py`: PyQt6 GUI 관제 프로그램 (samplecode 기반, 기능 유지)
- `src/userset.json`: 설정 파일 (실행 파일과 같은 폴더에서 읽음)
- `배포용/`: `checkserver.exe`, `userset.json`, `사용설명.txt`
- `samplecode/`: 원본 참고 파일
- `_archive_cli/`: 이전 CLI 버전 (보관용, 사용하지 않음)

## 주요 기능
- 대시보드: CPU / RAM / DISK / 프로세스 수 도넛 게이지, 임계치 초과 시 경고 테마와 비프음
- 프로세스 탭: 실시간 목록, 감시 대상 강조, 상위 5개 부하 표시, 더블클릭 시 실행 정보
- 점검실행 탭: 일일 FAS / EAI 점검 배치 실행, 콘솔 출력
- 결과조회 탭: 날짜별 점검 이력의 TXT / CSV 열람
- 로그 탭: 누적 로그(`C:\checkPMI\logs\system_accumulated.log`) 열람·검색
- 설정 탭: 감시 프로세스, 임계치, 드라이브, NTP 시간 동기화 명령, 배치 경로

## 실행 (개발 환경)
```
python src/CheckServer.py
```
- 필요 패키지: `PyQt6`, `psutil`
- 실행 시 `C:\checkPMI\` 아래에 기본 점검 배치 스크립트가 없으면 자동 생성됨

## 참고
- 상위 명세: `../code_artifact.md` 3.2절
- **공개 전 확인 필요:** `CheckServer.py`의 내장 배치 스크립트와 `userset.json`에 내부 IP(172.21.126.31)가 원본 그대로 들어 있음. 과제 보안 기준상 공개 저장소에서는 마스킹해야 함 (현재 미수정)
- 이전 CLI 진단기(`_archive_cli/`)의 지속 부하·권고 로직은 GUI에 포함되지 않음. 필요하면 별도 반영 가능
