//! Verify reset-command ownership and cancellation without a USB device.

#[path = "../../firmware/src/usb/control.rs"]
mod control;

use control::State;

fn connected() -> (State, u32) {
    let mut state = State::new();
    state.observe(7, true);
    let owner = state.owner(7).unwrap();
    (state, owner)
}

#[test]
fn a_timed_out_reply_cannot_start_or_continue_a_packet_write() {
    let (mut state, owner) = connected();
    let ticket = state.begin_reply(owner, 7).unwrap();
    assert!(state.reply_live(ticket));
    state.cancel_reply(ticket);
    assert!(!state.reply_live(ticket));
    assert!(!state.accepts_ack(ticket, ticket, true));
}

#[test]
fn a_new_ticket_rejects_late_completion_and_cancellation_from_the_old_ticket() {
    let (mut state, owner) = connected();
    let old = state.begin_reply(owner, 7).unwrap();
    state.cancel_reply(old);
    let current = state.begin_reply(owner, 7).unwrap();
    assert_ne!(old, current);
    state.cancel_reply(old);
    assert!(state.reply_live(current));
    assert!(!state.accepts_ack(current, old, true));
    assert!(!state.accepts_ack(old, old, true));
    assert!(state.accepts_ack(current, current, true));
}

#[test]
fn dtr_close_and_reopen_rejects_old_requests_on_the_same_usb_generation() {
    let (mut state, old_owner) = connected();
    let old_ticket = state.begin_reply(old_owner, 7).unwrap();
    state.disconnect(7);
    assert!(!state.connected(old_owner, 7));
    state.observe(7, true);
    let new_owner = state.owner(7).unwrap();
    assert_ne!(new_owner, old_owner);
    assert!(!state.connected(old_owner, 7));
    assert!(!state.reply_live(old_ticket));
    assert!(!state.accepts_ack(old_ticket, old_ticket, true));
    assert!(state.begin_reply(old_owner, 7).is_none());
    assert!(state.begin_reply(new_owner, 7).is_some());
}

#[test]
fn a_usb_generation_change_invalidates_control_ownership_and_acknowledgement() {
    let (mut state, old_owner) = connected();
    let ticket = state.begin_reply(old_owner, 7).unwrap();
    state.observe(8, true);
    let current_owner = state.owner(8).unwrap();
    assert!(!state.connected(old_owner, 7));
    assert!(!state.connected(old_owner, 8));
    assert!(!state.reply_live(ticket));
    assert!(!state.accepts_ack(ticket, ticket, true));
    state.disconnect(7);
    assert!(state.connected(current_owner, 8));
}

#[test]
fn an_ack_queued_before_timeout_or_disconnect_is_rejected_afterward() {
    for disconnect in [false, true] {
        let (mut state, owner) = connected();
        let ticket = state.begin_reply(owner, 7).unwrap();
        let queued_ack = (ticket, true);
        assert!(state.accepts_ack(ticket, queued_ack.0, queued_ack.1));
        if disconnect {
            state.disconnect(7);
        } else {
            state.cancel_reply(ticket);
        }
        assert!(!state.accepts_ack(ticket, queued_ack.0, queued_ack.1));
    }
}

#[test]
fn repeated_connected_observations_preserve_the_current_ticket() {
    let (mut state, owner) = connected();
    let ticket = state.begin_reply(owner, 7).unwrap();
    for _ in 0..10 {
        state.observe(7, true);
        assert_eq!(state.owner(7), Some(owner));
        assert!(state.reply_live(ticket));
    }
    assert!(state.begin_reply(owner, 7).is_none());
    assert!(!state.accepts_ack(ticket, ticket, false));
    assert!(state.accepts_ack(ticket, ticket, true));
}

#[test]
fn a_disconnected_observation_prevents_control_replies_until_a_new_session() {
    let (mut state, owner) = connected();
    state.observe(7, false);
    assert_eq!(state.owner(7), None);
    assert!(state.begin_reply(owner, 7).is_none());
    state.observe(7, true);
    assert!(!state.connected(owner, 7));
    let replacement = state.owner(7).unwrap();
    assert!(state.begin_reply(replacement, 7).is_some());
}

#[test]
fn a_queued_immediate_error_cannot_reach_a_reopened_cdc_session() {
    let (mut state, owner) = connected();
    // An immediate error has an owner but no acknowledgement ticket.
    assert!(state.reply_session_live(7, Some(owner)));
    state.disconnect(7);
    assert!(!state.reply_session_live(7, Some(owner)));
    state.observe(7, true);
    let replacement = state.owner(7).unwrap();
    assert!(!state.reply_session_live(7, Some(owner)));
    assert!(state.reply_session_live(7, Some(replacement)));
}

#[test]
fn a_session_change_stops_the_remaining_packets_of_an_immediate_reply() {
    let (mut state, owner) = connected();
    assert!(state.reply_session_live(7, Some(owner)));
    // The writer checks this same condition before each packet and after the last.
    state.observe(8, true);
    assert!(!state.reply_session_live(7, Some(owner)));
    assert!(!state.reply_session_live(8, Some(owner)));
    let replacement = state.owner(8).unwrap();
    assert!(state.reply_session_live(8, Some(replacement)));
}

#[test]
fn legacy_replies_keep_bus_generation_scope_without_a_control_owner() {
    let (mut state, _) = connected();
    assert!(state.reply_session_live(7, None));
    state.disconnect(7);
    state.observe(7, true);
    assert!(state.reply_session_live(7, None));
    state.observe(8, true);
    assert!(!state.reply_session_live(7, None));
    assert!(state.reply_session_live(8, None));
}
