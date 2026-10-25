//! Retain refresh measurements and encode bounded framebuffer chunks.

use super::power_sequence::Measurement;
use alloc::string::String;
use serde_json::{Value, json};

pub const MAX_CHUNK: usize = 512;

#[derive(Default)]
pub struct State {
    latest: Option<Refresh>,
}

struct Refresh {
    epoch: u32,
    started_ms: u64,
    finished_ms: Option<u64>,
    mode: &'static str,
    frame_checksum: u32,
    success: Option<bool>,
    shutdown_confirmed: Option<bool>,
    pmic: Option<Measurement>,
}

impl State {
    pub fn begin(&mut self, mode: &'static str, frame_checksum: u32, now_ms: u64) {
        let epoch = self
            .latest
            .as_ref()
            .map_or(1, |last| last.epoch.wrapping_add(1));
        self.latest = Some(Refresh {
            epoch,
            started_ms: now_ms,
            finished_ms: None,
            mode,
            frame_checksum,
            success: None,
            shutdown_confirmed: None,
            pmic: None,
        });
    }

    pub fn finish(
        &mut self,
        success: bool,
        shutdown_confirmed: bool,
        pmic: Option<Measurement>,
        now_ms: u64,
    ) {
        if let Some(latest) = self.latest.as_mut() {
            latest.finished_ms = Some(now_ms);
            latest.success = Some(success);
            latest.shutdown_confirmed = Some(shutdown_confirmed);
            latest.pmic = pmic;
        }
    }

    pub fn value(&self) -> Value {
        let Some(latest) = self.latest.as_ref() else {
            return Value::Null;
        };
        json!({
            "epoch": latest.epoch,
            "started_ms": latest.started_ms,
            "finished_ms": latest.finished_ms,
            "mode": latest.mode,
            "frame_checksum": latest.frame_checksum,
            "success": latest.success,
            "shutdown_confirmed": latest.shutdown_confirmed,
            "pmic": latest.pmic.map(pmic_value),
        })
    }
}

fn pmic_value(measured: Measurement) -> Value {
    json!({
        "source": "existing reads during refresh; not live",
        "attempt": measured.attempt,
        "started_ms": measured.started_ms,
        "ready_ms": measured.ready_ms,
        "shutdown_ms": measured.shutdown_ms,
        "revision": measured.revision,
        "temperature": measured.temperature,
        "enable_start": measured.enable_start,
        "vcom1": measured.vcom1,
        "vcom2": measured.vcom2,
        "sequence": measured.sequence,
        "faults_before": measured.faults_before,
        "faults_active": measured.faults_active,
        "power_good_start": measured.power_good_start,
        "power_good_active": measured.power_good_active,
        "power_good_off": measured.power_good_off,
        "enable_off": measured.enable_off,
    })
}

pub fn hex_chunk(bytes: &[u8], offset: usize, length: usize) -> Option<String> {
    if length == 0 || length > MAX_CHUNK {
        return None;
    }
    let end = offset.checked_add(length)?;
    let chunk = bytes.get(offset..end)?;
    let mut result = String::with_capacity(length * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in chunk {
        result.push(HEX[usize::from(byte >> 4)] as char);
        result.push(HEX[usize::from(byte & 15)] as char);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::{MAX_CHUNK, Measurement, State, hex_chunk};
    use serde_json::Value;

    #[test]
    fn chunks_preserve_native_byte_order_and_both_nibbles() {
        let bytes = [0xff, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0];
        assert_eq!(hex_chunk(&bytes, 1, 8).as_deref(), Some("0123456789abcdef"));
        assert_eq!(hex_chunk(&bytes, 9, 1).as_deref(), Some("00"));
        assert_eq!(
            bytes,
            [0xff, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0]
        );
    }

    #[test]
    fn chunks_reject_zero_oversize_overflow_and_out_of_range_requests() {
        let bytes = [0xab; MAX_CHUNK + 1];
        assert_eq!(
            hex_chunk(&bytes, 1, MAX_CHUNK).unwrap().len(),
            MAX_CHUNK * 2
        );
        for (offset, length) in [
            (0, 0),
            (0, MAX_CHUNK + 1),
            (2, MAX_CHUNK),
            (bytes.len(), 1),
            (usize::MAX, 1),
            (usize::MAX - 2, 4),
        ] {
            assert_eq!(hex_chunk(&bytes, offset, length), None);
        }
    }

    #[test]
    fn a_new_attempt_cannot_reuse_success_or_pmic_values_from_the_previous_frame() {
        let mut state = State::default();
        assert_eq!(state.value(), Value::Null);
        state.begin("full", 12, 100);
        state.finish(
            true,
            true,
            Some(Measurement {
                vcom1: Some(160),
                ..Measurement::default()
            }),
            200,
        );
        let previous = state.value();
        assert_eq!(previous["success"], true);
        assert_eq!(previous["pmic"]["vcom1"], 160);
        state.begin("partial", 34, 300);
        let pending = state.value();
        assert_eq!(pending["epoch"], 2);
        assert_eq!(pending["frame_checksum"], 34);
        assert_eq!(pending["success"], Value::Null);
        assert_eq!(pending["shutdown_confirmed"], Value::Null);
        assert_eq!(pending["pmic"], Value::Null);
        state.finish(false, true, None, 400);
        let failed = state.value();
        assert_eq!(failed["success"], false);
        assert_eq!(failed["shutdown_confirmed"], true);
        assert_eq!(failed["pmic"], Value::Null);
        assert_eq!(failed["finished_ms"], 400);
    }
}
