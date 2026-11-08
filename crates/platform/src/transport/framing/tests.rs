use super::{FrameError, LineFramer};

// A protocol limit below storage capacity must reject longer frames, including after reset.
#[test]
fn configured_limit_survives_completion_and_clear() {
    let mut frame = LineFramer::<8>::with_limit(2).unwrap();
    for _ in 0..2 {
        for byte in b"ab" {
            assert_eq!(frame.push(*byte), None);
        }
        assert_eq!(frame.push(b'\n').unwrap().unwrap().as_slice(), b"ab");
        for byte in b"abc" {
            assert_eq!(frame.push(*byte), None);
        }
        assert_eq!(frame.push(b'\n'), Some(Err(FrameError::TooLong)));
        frame.clear();
    }
}

// Configuration must reject limits that fixed storage cannot satisfy.
#[test]
fn configured_limit_cannot_exceed_capacity() {
    assert!(LineFramer::<8>::with_limit(9).is_none());
    let mut frame = LineFramer::<2>::with_limit(2).unwrap();
    assert_eq!(frame.push(b'a'), None);
    assert_eq!(frame.push(b'b'), None);
    assert_eq!(frame.push(b'\n').unwrap().unwrap().as_slice(), b"ab");
}

// A zero protocol limit must accept only empty frames even when storage is available.
#[test]
fn zero_limit_rejects_payload_with_spare_capacity() {
    let mut frame = LineFramer::<8>::with_limit(0).unwrap();
    assert!(frame.push(b'\n').unwrap().unwrap().is_empty());
    assert_eq!(frame.push(b'x'), None);
    assert_eq!(frame.push(b'\n'), Some(Err(FrameError::TooLong)));
}

// A frame at capacity must succeed because LF does not consume payload storage.
#[test]
fn capacity_excludes_the_delimiter() {
    let mut frame = LineFramer::<3>::default();
    for byte in b"abc" {
        assert_eq!(frame.push(*byte), None);
    }
    assert_eq!(frame.push(b'\n').unwrap().unwrap().as_slice(), b"abc");
}

// Overflow must discard the complete suffix and permit a fresh frame after LF.
#[test]
fn overflow_drains_to_the_next_delimiter() {
    let mut frame = LineFramer::<3>::default();
    for byte in b"abcdreset" {
        assert_eq!(frame.push(*byte), None);
    }
    assert_eq!(frame.push(b'\n'), Some(Err(FrameError::TooLong)));
    assert_eq!(frame.push(b'x'), None);
    assert_eq!(frame.push(b'\n').unwrap().unwrap().as_slice(), b"x");
}

// A new session must not inherit partial bytes or an overflow from the previous session.
#[test]
fn clear_discards_partial_and_overflow_state() {
    for input in [b"ab".as_slice(), b"abcd"] {
        let mut frame = LineFramer::<3>::default();
        for byte in input {
            assert_eq!(frame.push(*byte), None);
        }
        frame.clear();
        assert_eq!(frame.push(b'x'), None);
        assert_eq!(frame.push(b'\n').unwrap().unwrap().as_slice(), b"x");
    }
}

// Framing must preserve non-LF bytes and leave empty-frame policy to the caller.
#[test]
fn non_delimiter_bytes_and_empty_frames_are_preserved() {
    let mut frame = LineFramer::<4>::default();
    for byte in [0, 255, b'\r'] {
        assert_eq!(frame.push(byte), None);
    }
    assert_eq!(
        frame.push(b'\n').unwrap().unwrap().as_slice(),
        &[0, 255, b'\r']
    );
    assert!(frame.push(b'\n').unwrap().unwrap().is_empty());
}

// Zero capacity accepts empty frames but rejects any payload without a panic.
#[test]
fn zero_capacity_rejects_nonempty_frames() {
    let mut frame = LineFramer::<0>::default();
    assert!(frame.push(b'\n').unwrap().unwrap().is_empty());
    assert_eq!(frame.push(b'x'), None);
    assert_eq!(frame.push(b'\n'), Some(Err(FrameError::TooLong)));
}
