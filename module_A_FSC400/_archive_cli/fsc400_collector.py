"""FSC400 초음파 유량계 데이터 수집기 (MODBUS-RTU, 시뮬레이션 폴백 지원).

- 실장비: RS485 MODBUS-RTU (기능 코드 0x04, Input Register) 폴링 후 CSV 저장
- 시뮬레이션: 실장비 미연결 시 자동 폴백, 또는 --simulate 로 가상 파형 생성
- 레지스터 맵은 FSC400 매뉴얼 부록 A 기준 (float/uint32 워드 순서 CDAB)
"""

from __future__ import annotations

import argparse
import csv
import math
import random
import struct
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from typing import Iterator, Optional

try:
    import serial  # pyserial
except ImportError:  # 시뮬레이션 전용 실행 환경 대응
    serial = None

# 매뉴얼 부록 A: 순간 유량 0-1, 합산 누적 정수부 4-5, 합산 누적 소수부 16, 순간 유속 24-25
REG_START = 0
REG_COUNT = 26  # 0~25 한 번에 읽음
CSV_FIELDS = ["timestamp", "flow_m3h", "velocity_ms", "totalizer_m3"]


@dataclass
class Sample:
    timestamp: datetime
    flow_m3h: float
    velocity_ms: float
    totalizer_m3: float


# ---------- MODBUS-RTU 저수준 ----------

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
    # 워드 순서 CDAB: 수신 바이트 [C D A B] -> 표준 big-endian [A B C D]
    raw = struct.pack(">HH", regs[0], regs[1])
    return struct.unpack(">f", bytes([raw[2], raw[3], raw[0], raw[1]]))[0]


def decode_cdab_uint32(regs: list[int]) -> int:
    return (regs[1] << 16) | regs[0]


def parse_response(frame: bytes, slave: int, count: int) -> list[int]:
    expected = 3 + 2 * count + 2
    if len(frame) != expected:
        raise IOError(f"응답 길이 불일치: {len(frame)} != {expected}")
    if crc16_modbus(frame[:-2]) != struct.unpack("<H", frame[-2:])[0]:
        raise IOError("CRC 오류")
    if frame[0] != slave:
        raise IOError(f"슬레이브 주소 불일치: {frame[0]}")
    if frame[1] & 0x80:
        raise IOError(f"MODBUS 예외 응답: 코드 {frame[2]}")
    if frame[1] != 0x04 or frame[2] != 2 * count:
        raise IOError("기능 코드 또는 바이트 수 불일치")
    return [struct.unpack(">H", frame[3 + i:5 + i])[0] for i in range(0, 2 * count, 2)]


def regs_to_sample(regs: list[int]) -> Sample:
    base = regs[0:26]  # 주소 0~25
    flow = decode_cdab_float(base[0:2])
    velocity = decode_cdab_float(base[24:26])
    totalizer = decode_cdab_uint32(base[4:6]) + base[16] * 0.001
    return Sample(datetime.now(), flow, velocity, totalizer)


# ---------- 수집 소스 ----------

class LiveSource:
    """RS485 실장비에서 주기적으로 레지스터를 폴링한다."""

    def __init__(self, port: str, baud: int, slave: int, timeout: float = 0.5) -> None:
        if serial is None:
            raise RuntimeError("pyserial 미설치")
        self.slave = slave
        self.ser = serial.Serial(port, baudrate=baud, bytesize=8,
                                 parity=serial.PARITY_NONE, stopbits=1, timeout=timeout)

    def read(self) -> Sample:
        self.ser.reset_input_buffer()
        self.ser.write(build_read_request(self.slave, REG_START, REG_COUNT))
        expected = 3 + 2 * REG_COUNT + 2
        frame = self.ser.read(expected)
        return regs_to_sample(parse_response(frame, self.slave, REG_COUNT))

    def close(self) -> None:
        self.ser.close()


class SimulatedSource:
    """정상 맥동 파형 + 랜덤 스파이크 노이즈를 생성한다 (샘플 데이터 생성에도 사용)."""

    def __init__(self, rate_hz: float = 10.0, seed: Optional[int] = 42,
                 spike_prob: float = 0.005, start: Optional[datetime] = None) -> None:
        self.dt = 1.0 / rate_hz
        self.rng = random.Random(seed)
        self.spike_prob = spike_prob
        self.t = 0.0
        self.now = start or datetime(2026, 1, 1, 0, 0, 0)
        self.totalizer = 1000.0

    def read(self) -> Sample:
        t = self.t
        # 기본 유량 + 저주파 변동 + 2.5 Hz 펌프 맥동 + 측정 노이즈
        flow = 200.0 + 15.0 * math.sin(2 * math.pi * 0.05 * t) \
            + 4.0 * math.sin(2 * math.pi * 2.5 * t) + self.rng.gauss(0, 0.3)
        if self.rng.random() < self.spike_prob:
            flow += self.rng.choice([-1, 1]) * self.rng.uniform(40, 90)  # 통신 오류/배관 충격 스파이크
        velocity = flow / 3600.0 / (math.pi * (0.1 / 2) ** 2)  # 가상 관경 DN100 기준
        self.totalizer += max(flow, 0) / 3600.0 * self.dt
        sample = Sample(self.now, round(flow, 3), round(velocity, 4), round(self.totalizer, 3))
        self.t += self.dt
        self.now += timedelta(seconds=self.dt)
        return sample

    def close(self) -> None:
        pass


# ---------- 출력 ----------

def write_csv(path: Path, samples: Iterator[Sample]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(CSV_FIELDS)
        for s in samples:
            writer.writerow([s.timestamp.isoformat(timespec="milliseconds"),
                             s.flow_m3h, s.velocity_ms, s.totalizer_m3])
            count += 1
    return count


def collect(source, duration: float, interval: float, out: Path) -> int:
    def gen() -> Iterator[Sample]:
        end = time.monotonic() + duration
        while time.monotonic() < end:
            tick = time.monotonic()
            try:
                yield source.read()
            except (IOError, OSError) as exc:
                print(f"[경고] 샘플 수집 실패, 건너뜀: {exc}", file=sys.stderr)
            time.sleep(max(0.0, interval - (time.monotonic() - tick)))
    return write_csv(out, gen())


def simulate_to_csv(samples: int, rate_hz: float, out: Path, seed: int) -> int:
    src = SimulatedSource(rate_hz=rate_hz, seed=seed)
    return write_csv(out, (src.read() for _ in range(samples)))


def open_live_or_none(args: argparse.Namespace) -> Optional[LiveSource]:
    try:
        src = LiveSource(args.port, args.baud, args.slave)
        src.read()  # 연결 확인용 1회 읽기
        return src
    except Exception as exc:  # 포트 없음, 응답 없음 등 모두 폴백 대상
        print(f"[폴백] 실장비 연결 실패 ({exc}) -> 시뮬레이션 모드로 전환", file=sys.stderr)
        return None


def main(argv: Optional[list[str]] = None) -> int:
    p = argparse.ArgumentParser(description="FSC400 유량계 수집기")
    mode = p.add_mutually_exclusive_group()
    mode.add_argument("--live", action="store_true", help="실장비 RS485 폴링")
    mode.add_argument("--simulate", action="store_true", help="가상 파형 생성")
    p.add_argument("--port", default="COM3", help="시리얼 포트 (예: COM3)")
    p.add_argument("--baud", type=int, default=9600)
    p.add_argument("--slave", type=int, default=1, help="MODBUS 슬레이브 주소")
    p.add_argument("--duration", type=float, default=10.0, help="--live 수집 시간(초)")
    p.add_argument("--interval", type=float, default=0.1, help="--live 폴링 주기(초)")
    p.add_argument("--samples", type=int, default=1000, help="--simulate 샘플 수")
    p.add_argument("--rate", type=float, default=10.0, help="--simulate 샘플링 주파수(Hz)")
    p.add_argument("--seed", type=int, default=42)
    p.add_argument("--output", type=Path, default=Path("outputs/fsc400_collected.csv"))
    args = p.parse_args(argv)

    if args.live:
        src = open_live_or_none(args)
        if src is None:
            n = simulate_to_csv(int(args.duration / args.interval), 1 / args.interval, args.output, args.seed)
        else:
            try:
                n = collect(src, args.duration, args.interval, args.output)
            finally:
                src.close()
        mode_name = "live" if src is not None else "simulate(fallback)"
    else:
        n = simulate_to_csv(args.samples, args.rate, args.output, args.seed)
        mode_name = "simulate"

    print(f"[{mode_name}] {n}행 저장 -> {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
