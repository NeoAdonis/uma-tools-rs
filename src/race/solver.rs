use crate::course::Course;
use crate::race::parameters::RaceParameters;
use crate::race::runner::Runner;
use crate::uma::skill::*;
use crate::uma::*;
use rand::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// TODO: implement blocking logic
// When blocked, a uma’s speed is capped between 0.988x(at 0m) to 1.0x(at 2m) of the speed of the uma in front, scaled linearly to the distance in between.
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.eyiunid0e18k

// TODO: implement lane change logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.ohdxzwro17n5

// TODO: implement start delay logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.iq42ksqbg2p

// TODO: implement position keep logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.tulele26d1pm

// TODO: implement rush logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.awbjufvej6ud

pub struct RaceState {
    pub course: Course,
    pub params: RaceParameters,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    pub elapsed_time: f64, // Race time in seconds (shared by all umas)
}

pub struct RaceSolver {
    pub runners: Vec<Runner>,
    pub state: RaceState,
}

impl RaceSolver {
    pub fn new(course: Course, params: RaceParameters) -> Self {
        let seed = params.rng_seed;
        let rng = Rc::new(RefCell::new(rand_pcg::Pcg64Mcg::seed_from_u64(seed)));

        let state = RaceState {
            course,
            params,
            rng,
            elapsed_time: 0.0,
        };

        RaceSolver {
            runners: vec![],
            state,
        }
    }

    pub fn add_runner(&mut self, uma: Uma, motivation: Motivation, skills: Vec<Skill>) {
        let runner = Runner::new(
            uma,
            motivation,
            &self.state.course,
            &self.state.params,
            skills,
            Rc::clone(&self.state.rng),
        );
        self.runners.push(runner);
    }

    // TODO: complete implementation and report
    #[allow(dead_code)]
    fn displayed_time(&self) -> f64 {
        self.state.elapsed_time * 1.18
    }

    pub fn step(&mut self, delta_time: f64) {
        // Pre-Update
        for runner in &mut self.runners {
            runner.check_corner_end(&self.state.course.corners);
            runner.check_straight_end(&self.state.course.straights);
        }

        self.state.elapsed_time += delta_time;

        for runner in &mut self.runners {
            // Update
            runner.check_time_triggers(&self.state);
            // TODO: Check random activation triggers
            // TODO: Check continuous triggers
            // TODO: implement "Recover HP by skills"
            runner.update_last_spurt_state(&self.state.course, self.state.params.condition);
            runner.update_target_speed(self.state.course.base_speed()); // TODO: review
            runner.update_acceleration();
            runner.update_phase(&self.state);
            runner.update_speed(delta_time, self.state.course.base_speed());
            runner.update_position(delta_time, &self.state);
            runner.update_hp(delta_time, &self.state);
            runner.update_slope(&self.state.course.slopes, &mut *self.state.rng.borrow_mut());
            runner.update_skill_durations(self.state.elapsed_time);
            // Post-Update
            runner.check_corner_start(&self.state.course.corners);
            runner.check_straight_start(&self.state.course.straights);
        }
    }

    pub fn run(&mut self) {
        let dt = 1.0 / 15.0; // 15 FPS
        // TODO: replace `for` with `while`
        //while self.runner.position < self.course.distance as f64 {
        for _ in 0..1 {
            self.step(dt);
        }
    }
}
