//! MODBUS-RTU 프레이밍과 FSC400 레지스터 디코딩 (매뉴얼 부록 A).

use chrono::{DateTime, Local};

/// 주소 0~25를 한 번에 읽는다 (순간 유량 0-1, 적산 4-5, 소수부 16, 유속 24-25).
pub const REG_COUNT: u16 = 26;

#[derive(Clone, Debug)]
pub struct Sample {
    pub timestamp: DateTime<Local>,
    pub header: String,
    pub flow: f64,
    pub total: f64,
    pub velocity: f64,
    pub error: String,
}

pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc
}

/// 기능 코드 0x04 (Read Input Registers) 요청 프레임.
pub fn build_read_request(slave: u8, start: u16, count: u16) -> Vec<u8> {
    let mut body = vec![slave, 0x04];
    body.extend_from_slice(&start.to_be_bytes());
    body.extend_from_slice(&count.to_be_bytes());
    let crc = crc16_modbus(&body);
    body.extend_from_slice(&crc.to_le_bytes());
    body
}

/// 워드 순서 CDAB: 수신 바이트 [C D A B]를 [A B C D]로 재배열하면 w1 상위, w0 하위 워드.
pub fn decode_cdab_float(w0: u16, w1: u16) -> f32 {
    f32::from_bits(((w1 as u32) << 16) | w0 as u32)
}

/// 응답 프레임 검사 후 레지스터 값(워드) 목록을 돌려준다.
pub fn parse_response(frame: &[u8], slave: u8, count: u16) -> Result<Vec<u16>, String> {
    let expected = 3 + 2 * count as usize + 2;
    if frame.len() != expected {
        return Err(format!("응답 길이 불일치 ({}/{})", frame.len(), expected));
    }
    let (payload, crc_bytes) = frame.split_at(frame.len() - 2);
    let received = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
    if crc16_modbus(payload) != received {
        return Err("CRC 오류".to_string());
    }
    if frame[0] != slave {
        return Err(format!("슬레이브 주소 불일치 ({})", frame[0]));
    }
    if frame[1] & 0x80 != 0 {
        return Err(format!("MODBUS 예외 응답 (코드 {})", frame[2]));
    }
    if frame[1] != 0x04 || frame[2] as usize != 2 * count as usize {
        return Err("기능 코드 또는 바이트 수 불일치".to_string());
    }
    Ok(frame[3..3 + 2 * count as usize]
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect())
}

pub fn regs_to_sample(regs: &[u16]) -> Sample {
    let flow = decode_cdab_float(regs[0], regs[1]) as f64;
    let velocity = decode_cdab_float(regs[24], regs[25]) as f64;
    let total = (((regs[5] as u32) << 16) | regs[4] as u32) as f64 + regs[16] as f64 * 0.001;
    Sample {
        timestamp: Local::now(),
        header: "FSC400".to_string(),
        flow,
        total,
        velocity,
        error: "N/A".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_manual_example() {
        assert_eq!(crc16_modbus(&[0x01, 0x04, 0x00, 0x00, 0x00, 0x28]), 0x14F0);
    }

    #[test]
    fn request_matches_expected_frame() {
        assert_eq!(
            build_read_request(1, 0, 26),
            vec![0x01, 0x04, 0x00, 0x00, 0x00, 0x1A, 0x71, 0xC1]
        );
    }

    #[test]
    fn cdab_decodes_manual_example() {
        let v = decode_cdab_float(0x82D0, 0x44FE);
        assert!((v as f64 - 2036.087890625).abs() < 1e-6, "{v}");
    }

    #[test]
    fn response_round_trip() {
        let mut regs = vec![0u16; 26];
        regs[0] = 0x82D0;
        regs[1] = 0x44FE;
        regs[4] = 0x00C9;
        let mut body = vec![1u8, 4, 52];
        for r in &regs {
            body.extend_from_slice(&r.to_be_bytes());
        }
        let crc = crc16_modbus(&body);
        body.extend_from_slice(&crc.to_le_bytes());
        let parsed = parse_response(&body, 1, 26).unwrap();
        let s = regs_to_sample(&parsed);
        assert!((s.flow - 2036.087890625).abs() < 1e-6);
        assert_eq!(s.total, 201.0);
    }

    #[test]
    fn bad_crc_is_rejected() {
        let frame = vec![1u8, 4, 52, 0, 0, 0, 0, 0, 0];
        assert!(parse_response(&frame, 1, 26).is_err());
    }
}
