//! Every failure the protocol can meet, on the simulated keyboard: the session reports the right error, retries
//! reads only when that is safe, and never stores anything in flash it should not.

use filco_core::{Page, SlotAddressing};
use filco_protocol::{
    CMD_READ_PAGE, CMD_WRITE_PAGE, Delays, Phase, PolicyTransport, ProtocolError, Session, TransportError,
    VirtualClock, WritePolicy,
};
use filco_sim::{Fault, FaultKind, SimKeyboard, Stage};

fn page_with(byte: u8) -> Page {
    let mut p = Page::empty();
    for i in 0..128u8 {
        p.set_slot(
            SlotAddressing::CostarInterleaved8,
            i,
            filco_core::Slot([2, 0, byte, 0]),
        );
    }
    p
}

fn sim() -> SimKeyboard {
    SimKeyboard::new((0..4).map(|l| page_with(0x10 + l)).collect())
}

fn session(sim: SimKeyboard) -> Session<SimKeyboard, VirtualClock> {
    Session::new(sim, VirtualClock::default(), Delays::FILCO_ASSIST)
}

fn fault(cmd: u8, nth: usize, stage: Stage, kind: FaultKind) -> Fault {
    Fault {
        cmd,
        nth,
        stage,
        kind,
    }
}

#[test]
fn read_progress_counts_every_report() {
    let mut s = session(sim());
    let mut seen = Vec::new();
    s.read_page(2, &mut |p| seen.push((p.layer, p.phase, p.done, p.total)))
        .unwrap();
    // ACK after 2 reports, one step per data report, then the finish packet and its ACK.
    let want: Vec<_> = (2..=10).chain([12]).map(|d| (2, Phase::Read, d, 12)).collect();
    assert_eq!(seen, want);
}

#[test]
fn read_retries_after_wrong_ack_and_bad_checksum() {
    for (stage, kind) in [
        (Stage::Command, FaultKind::WrongAck),
        (Stage::Finish, FaultKind::BadChecksum),
        (Stage::Finish, FaultKind::WrongAck),
        (Stage::Data { n: 5 }, FaultKind::CorruptData),
    ] {
        let mut s = session(sim().with_fault(fault(CMD_READ_PAGE, 0, stage, kind)));
        let p = s
            .read_page(1, &mut |_| {})
            .unwrap_or_else(|e| panic!("{kind:?} at {stage:?}: {e}"));
        assert_eq!(p, page_with(0x11), "{kind:?}");
        assert_eq!(s.transport().pending_faults(), &[]);
    }
}

#[test]
fn read_gives_up_after_three_failures() {
    let mut sim = sim();
    for nth in 0..3 {
        sim.add_fault(fault(CMD_READ_PAGE, nth, Stage::Finish, FaultKind::BadChecksum));
    }
    let mut s = session(sim);
    assert!(matches!(
        s.read_page(0, &mut |_| {}),
        Err(ProtocolError::Checksum { .. })
    ));
}

#[test]
fn unplugging_during_any_read_report_fails_without_retry() {
    let stages = [Stage::Command, Stage::Finish]
        .into_iter()
        .chain((0..8).map(|n| Stage::Data { n }));
    for stage in stages {
        let mut s = session(sim().with_fault(fault(CMD_READ_PAGE, 0, stage, FaultKind::Disconnect)));
        let r = s.read_page(0, &mut |_| {});
        assert_eq!(
            r,
            Err(ProtocolError::Transport(TransportError::Disconnected)),
            "{stage:?}"
        );
        assert!(!s.transport().is_connected());
    }
}

#[test]
fn io_error_is_reported() {
    let mut s = session(sim().with_fault(fault(CMD_READ_PAGE, 0, Stage::Data { n: 2 }, FaultKind::IoError)));
    assert!(matches!(
        s.read_page(0, &mut |_| {}),
        Err(ProtocolError::Transport(TransportError::Io { .. }))
    ));
}

#[test]
fn write_succeeds_and_reports_progress_then_verify() {
    let mut s = session(sim());
    let new = page_with(0x55);
    let mut phases = Vec::new();
    s.write_page(3, &new, &mut |p| phases.push((p.phase, p.done)))
        .unwrap();
    assert_eq!(s.transport().flash()[3], new);
    assert_eq!(phases.first(), Some(&(Phase::Write, 2)));
    assert_eq!(phases.last(), Some(&(Phase::Verify, 12)));
    assert_eq!(phases.iter().filter(|p| p.0 == Phase::Write).count(), 10);
    assert_eq!(phases.iter().filter(|p| p.0 == Phase::Verify).count(), 10);
}

#[test]
fn unplugging_before_the_finish_stores_nothing() {
    let stages = [Stage::Command, Stage::Finish, Stage::Commit]
        .into_iter()
        .chain((0..8).map(|n| Stage::Data { n }));
    for stage in stages {
        let before = sim();
        let mut s = session(before.clone().with_fault(fault(
            CMD_WRITE_PAGE,
            0,
            stage,
            FaultKind::Disconnect,
        )));
        let r = s.write_page(1, &page_with(0x99), &mut |_| {});
        assert_eq!(
            r,
            Err(ProtocolError::Transport(TransportError::Disconnected)),
            "{stage:?}"
        );
        assert_eq!(
            s.transport().flash(),
            before.flash(),
            "flash changed after unplug at {stage:?}"
        );
        assert_eq!(s.transport().flash_writes(), 0);
    }
}

#[test]
fn write_without_ack_sends_no_data() {
    let mut s = session(sim().with_fault(fault(CMD_WRITE_PAGE, 0, Stage::Command, FaultKind::WrongAck)));
    let r = s.write_page(0, &page_with(0x99), &mut |_| {});
    assert!(matches!(
        r,
        Err(ProtocolError::NoAck {
            cmd: CMD_WRITE_PAGE,
            ..
        })
    ));
    assert_eq!(
        s.transport().transcript().len(),
        2,
        "only the command and its reply"
    );
    assert_eq!(s.transport().flash_writes(), 0);
}

#[test]
fn write_checksum_mismatch_is_reported_and_never_retried() {
    let mut s = session(sim().with_fault(fault(CMD_WRITE_PAGE, 0, Stage::Finish, FaultKind::BadChecksum)));
    let r = s.write_page(0, &page_with(0x99), &mut |_| {});
    assert!(matches!(r, Err(ProtocolError::Checksum { .. })));
    assert_eq!(
        s.transport().transcript().len(),
        12,
        "one session, no read-back, no retry"
    );
}

#[test]
fn readback_mismatch_is_detected() {
    let mut s = session(sim().with_fault(fault(CMD_WRITE_PAGE, 0, Stage::Commit, FaultKind::CorruptCommit)));
    let r = s.write_page(2, &page_with(0x99), &mut |_| {});
    assert_eq!(
        r,
        Err(ProtocolError::Readback {
            layer: 2,
            first_diff: 256
        })
    );
}

#[test]
fn readback_that_fails_to_read_is_an_error() {
    // The write's own read-back is read session 0.
    let mut s = session(sim().with_fault(fault(
        CMD_READ_PAGE,
        0,
        Stage::Data { n: 7 },
        FaultKind::Disconnect,
    )));
    let r = s.write_page(2, &page_with(0x99), &mut |_| {});
    assert_eq!(r, Err(ProtocolError::Transport(TransportError::Disconnected)));
    assert_eq!(s.transport().flash_writes(), 1);
}

#[test]
fn read_only_policy_blocks_a_write_before_the_device_sees_anything() {
    let guarded = PolicyTransport::new(sim(), WritePolicy::ReadOnly);
    let mut s = Session::new(guarded, VirtualClock::default(), Delays::FILCO_ASSIST);
    let r = s.write_page(0, &page_with(0x99), &mut |_| {});
    assert!(matches!(
        r,
        Err(ProtocolError::Transport(TransportError::Refused(_)))
    ));
    assert_eq!(s.transport().inner().transcript().len(), 0);
    // Reading still works under the same policy.
    assert_eq!(s.read_page(0, &mut |_| {}).unwrap(), page_with(0x10));
}

#[test]
fn reconnect_recovers() {
    let mut s = session(sim().with_fault(fault(
        CMD_READ_PAGE,
        0,
        Stage::Data { n: 0 },
        FaultKind::Disconnect,
    )));
    assert!(s.read_page(0, &mut |_| {}).is_err());
    s.transport_mut().reconnect();
    assert_eq!(s.read_page(0, &mut |_| {}).unwrap(), page_with(0x10));
}

#[test]
fn unknown_layer_is_not_acknowledged() {
    let mut s = session(sim());
    assert!(matches!(
        s.read_page(9, &mut |_| {}),
        Err(ProtocolError::NoAck { .. })
    ));
}

#[test]
fn faults_round_trip_through_json() {
    let f = fault(CMD_WRITE_PAGE, 1, Stage::Data { n: 3 }, FaultKind::Disconnect);
    let text = serde_json::to_string(&f).unwrap();
    assert_eq!(
        text,
        r#"{"cmd":17,"nth":1,"stage":"data","n":3,"kind":"disconnect"}"#
    );
    assert_eq!(serde_json::from_str::<Fault>(&text).unwrap(), f);
}
