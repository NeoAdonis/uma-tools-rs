use super::*;
use crate::Entrant;
use crate::race::all_out_spurt::AllOutSpurt;
use crate::race::charge_up::ChargeUpOutcome;
use crate::race::compete_before_spurt;
use crate::race::secure_lead;
use crate::race::solver::{DELTA_TIME, RaceSolver};
use crate::race::spot_struggle::SpotStruggle;
use crate::skill::SkillType;
use crate::test_utils::*;
use crate::uma::{BaseStyle, Uma};
use std::collections::HashSet;
use std::mem::discriminant;
use std::sync::Arc;

/// Races a mixed field on a 3000m course and returns the finished solver.
///
/// Every runner is eligible for the mechanics of strong runners (charge up,
/// all-out spurt, stamina limit break) and has a recovery skill in the Late
/// phase. Every third runner rushes often, and every fourth one runs out of HP.
fn mixed_race(seed: u64) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10810), race_params(), seeded_config(seed));
    let heal = Arc::new(synthetic_skill(vec![alternative(
        "phase==2",
        SkillType::Recovery,
        0.05,
    )]));
    let styles = [
        BaseStyle::Front,
        BaseStyle::Pace,
        BaseStyle::Late,
        BaseStyle::End,
    ];
    for (i, &style) in styles.iter().cycle().take(9).enumerate() {
        let uma = Uma {
            speed: 3000,
            stamina: if i % 4 == 3 { 700 } else { 1800 },
            power: 1400,
            wit: if i % 3 == 0 { 100 } else { 1000 },
            ..uma(style)
        };
        solver.add_runner(Entrant::new(uma, vec![heal.clone()]));
    }
    let mut solver = solver.ready_runners();
    solver.run();
    solver
}

/// Asserts that `end` is the first frame at or past `expires_at`.
fn assert_first_frame_from(end: Option<EventEnd>, expires_at: f64, what: &str) {
    let end = end.unwrap_or_else(|| panic!("{what} didn't end"));
    assert!(
        end.time >= expires_at && end.time < expires_at + DELTA_TIME,
        "{what}: ended at {} instead of the first frame from {expires_at}",
        end.time
    );
}

/// Checks the events of `runner` against the state of each mechanic, and
/// returns the discriminants of the kinds of events she went through.
fn check_events(runner: &Runner) -> HashSet<std::mem::Discriminant<EventKind>> {
    let events = runner.events();
    let of = |is_kind: fn(&EventKind) -> bool| -> Vec<&Event> {
        events.iter().filter(|e| is_kind(&e.kind)).collect()
    };

    // In order of start, and ending after they start.
    for pair in events.windows(2) {
        assert!(pair[0].start_time <= pair[1].start_time);
    }
    for event in events {
        if let Some(end) = event.end {
            assert!(end.time >= event.start_time, "{event:?}");
            assert!(end.distance >= event.start_distance, "{event:?}");
        }
    }

    // Rush: the times of its state, and the style she rushed as.
    let rushes = of(|k| matches!(k, EventKind::Rush { .. }));
    let rush_state = runner.rush_state.as_ref().filter(|s| s.has_rushed());
    assert_eq!(rushes.len(), rush_state.iter().count());
    if let (Some(rush), Some(state)) = (rushes.first(), rush_state) {
        assert_eq!(rush.start_time, state.start_time);
        assert_eq!(rush.kind, EventKind::Rush { style: state.style });
        let expected = (!state.is_rushed).then_some(state.end_time);
        assert_eq!(rush.end.map(|end| end.time), expected);
    }

    // Spot struggle: until it expires, unless she exited before.
    let struggles = of(|k| matches!(k, EventKind::SpotStruggle));
    assert_eq!(
        struggles.len(),
        runner.spot_struggle.has_struggled() as usize
    );
    if let Some(struggle) = struggles.first() {
        if let SpotStruggle::Active { expires_at, .. } = runner.spot_struggle {
            assert_first_frame_from(struggle.end, expires_at, "spot struggle");
        } else {
            assert!(struggle.end.is_some());
        }
    }

    let duels = of(|k| matches!(k, EventKind::Duel));
    assert_eq!(duels.len(), runner.duel.has_dueled() as usize);
    if let Some(duel) = duels.first() {
        assert_eq!(duel.end.is_some(), runner.duel.has_ended());
    }

    // Charge up: the release's own start, and its end at expiry.
    let releases = of(|k| matches!(k, EventKind::ChargeUpRelease { .. }));
    let release = runner.charge_up.as_ref().and_then(|c| match &c.outcome {
        ChargeUpOutcome::Released(release) => Some(release),
        _ => None,
    });
    assert_eq!(releases.len(), release.iter().count());
    if let (Some(event), Some(release)) = (releases.first(), release) {
        assert_eq!(event.start_time, release.start_time);
        assert_eq!(event.start_distance, release.start_position);
        let kind = EventKind::ChargeUpRelease {
            conserved: release.conserved,
            acceleration: release.acceleration,
        };
        assert_eq!(event.kind, kind);
        if release.expires_at <= runner.finish_time().unwrap() {
            assert_first_frame_from(event.end, release.expires_at, "charge up release");
        }
    }

    // Timed boosts last their duration, then wait for their cooldown.
    for (kind, duration, cooldown) in [
        (
            EventKind::SecureLead,
            secure_lead::DURATION,
            secure_lead::COOLDOWN,
        ),
        (
            EventKind::CompeteBeforeSpurt,
            compete_before_spurt::DURATION,
            compete_before_spurt::COOLDOWN,
        ),
    ] {
        let boosts: Vec<_> = events.iter().filter(|e| e.kind == kind).collect();
        for boost in &boosts {
            assert_first_frame_from(boost.end, boost.start_time + duration, "timed boost");
        }
        for pair in boosts.windows(2) {
            assert!(pair[1].start_time >= pair[0].start_time + duration + cooldown);
        }
    }

    // A recovery skill ends every stamina keep, the last one if she gets none after
    // it.
    let keeps = of(|k| matches!(k, EventKind::StaminaKeep));
    assert!(keeps.len() <= 1 || keeps[..keeps.len() - 1].iter().all(|k| k.end.is_some()));
    assert_eq!(
        keeps.last().is_some_and(|k| k.end.is_none()),
        runner.stamina_keep.is_active
    );

    // The last spurt starts once, with no end, even if recalculated.
    let spurts = of(|k| matches!(k, EventKind::LastSpurt));
    let last_spurt = runner.last_spurt().unwrap();
    assert_eq!(spurts.len(), 1);
    assert!(spurts[0].end.is_none());
    assert!(spurts[0].start_distance >= last_spurt.target_distance - 100.0);

    let breaks = of(|k| matches!(k, EventKind::StaminaLimitBreak { .. }));
    assert_eq!(breaks.len(), (runner.stamina_limit_break > 0.0) as usize);
    if let Some(event) = breaks.first() {
        let kind = EventKind::StaminaLimitBreak {
            speed_bonus: runner.stamina_limit_break,
        };
        assert_eq!(event.kind, kind);
        assert!(event.end.is_none());
    }

    let all_out = of(|k| matches!(k, EventKind::AllOutSpurt));
    assert_eq!(
        all_out.len(),
        (runner.all_out_spurt != AllOutSpurt::Pending) as usize
    );
    if let Some(event) = all_out.first() {
        assert_eq!(event.end.is_some(), !runner.all_out_spurt.is_active());
    }

    events.iter().map(|e| discriminant(&e.kind)).collect()
}

#[test]
fn test_events_match_mechanic_state() {
    let mut seen = HashSet::new();
    let mut ended_keeps = 0;
    for seed in 0..8 {
        let solver = mixed_race(seed);
        for runner in solver.runners() {
            seen.extend(check_events(runner));
            let keeps = runner
                .events()
                .iter()
                .filter(|e| e.kind == EventKind::StaminaKeep);
            ended_keeps += keeps.filter(|e| e.end.is_some()).count();
        }
    }
    assert!(ended_keeps > 0, "no stamina keep ended");

    // Every kind of event happened, so none was checked in vain.
    let kinds = [
        EventKind::Rush {
            style: EffectiveStyle::Front,
        },
        EventKind::SpotStruggle,
        EventKind::Duel,
        EventKind::ChargeUpRelease {
            conserved: 0.0,
            acceleration: 0.0,
        },
        EventKind::CompeteBeforeSpurt,
        EventKind::StaminaKeep,
        EventKind::SecureLead,
        EventKind::LastSpurt,
        EventKind::AllOutSpurt,
        EventKind::StaminaLimitBreak { speed_bonus: 0.0 },
    ];
    for kind in kinds {
        assert!(seen.contains(&discriminant(&kind)), "no {kind:?} happened");
    }
}

#[test]
fn test_log_end_closes_the_last_open_event_of_its_kind() {
    let (mut runner, _) = ready_runner(EffectiveStyle::Front, seeded_config(0));
    let boost = |time| Event::start(EventKind::SecureLead, time, 0.0);
    let rush = |style| EventKind::Rush { style };
    runner.position = 20.0;

    // Nothing is open: no change.
    runner.log_end(EventKind::SecureLead, 5.0);
    assert!(runner.events().is_empty());

    runner.events = vec![
        boost(1.0),
        Event::start(rush(EffectiveStyle::Pace), 1.5, 2.0),
        boost(2.0),
    ];
    runner.log_end(EventKind::SecureLead, 3.0);
    let ends: Vec<_> = runner.events().iter().map(|e| e.end).collect();
    let end = EventEnd {
        time: 3.0,
        distance: 20.0,
    };
    assert_eq!(ends, [None, None, Some(end)]);

    // The payload doesn't matter, and ended events aren't ended again.
    runner.log_end(rush(EffectiveStyle::Front), 4.0);
    runner.log_end(EventKind::SecureLead, 5.0);
    runner.log_end(EventKind::SecureLead, 6.0);
    let times: Vec<_> = runner
        .events()
        .iter()
        .map(|e| e.end.map(|e| e.time))
        .collect();
    assert_eq!(times, [Some(5.0), Some(4.0), Some(3.0)]);
}
