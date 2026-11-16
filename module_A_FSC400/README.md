# 모듈 A: 산업용 유량계(FSC400) 실시간 관제 및 이력 조회

- **과목:** 인공지능특론세미나 (01반) 기말과제
- **학교:** 한양사이버대학교 일반대학원 기계IT융합공학과
- **학번 / 이름:** 2025300378 / 이민호

## 구성
- `src/fsc400_dashboard.py`: 실시간 관제 GUI (ttkbootstrap + matplotlib, 표준 수신 스레드 + 큐 구조)
- `docs/protocol_FSC400.md`: 매뉴얼 기반 MODBUS-RTU 레지스터 맵과 미확인 사항
- `배포용/fsc400_dashboard.exe`, `사용설명.txt`
- `samplecode/`: 원본 참고 파일 (`dashboard.py`, `makedata.py`)
- `_archive_cli/`: 이전 CLI 수집기와 그 배포 폴더 (보관용, 사용하지 않음)

## 주요 기능
- 소스 선택: MODBUS-RTU(매뉴얼 부록 A) / ASCII 라인 / 시뮬레이션
- 수신 데이터는 1초 단위로 일별 CSV(`data/flow_log_YYYY-MM-DD.csv`)에 기록
- 실시간 차트, 수치 카드, 목록 표시
- 달력으로 과거 기록 조회, 차트 클릭 시 목록 행 연동, 500건 단위 페이지 이동

## 실행 (개발 환경)
```
python src/fsc400_dashboard.py
```
- 필요 패키지: `ttkbootstrap`, `matplotlib`, `pyserial`

## 검증
- MODBUS 프레이밍(CRC, 응답 파싱, CDAB 디코딩)은 매뉴얼 예제로 확인
- 시뮬레이션 수신, 실시간 목록·차트 갱신, 일별 CSV 기록, 과거 기록 로드, 페이지 이동, 차트 클릭 연동을 자동 스크립트로 확인
- 실제 FSC400 장비 연결 및 ASCII 라인 장비 출력은 미확인

## 참고
- 상위 명세: `../code_artifact.md` 3.1절
- samplecode의 ASCII 라인 형식과 매뉴얼의 MODBUS 방식이 다르므로 실장비 확인이 필요함
- 에러 코드는 MODBUS 레지스터 맵에 없어 "N/A"로 표시됨
