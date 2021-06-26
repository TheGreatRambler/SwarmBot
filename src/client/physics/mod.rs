use num::traits::Pow;

use crate::client::physics::speed::Speed;
use crate::storage::block::{BlockApprox, BlockKind, BlockLocation, SimpleType};
use crate::storage::blocks::WorldBlocks;
use crate::types::{Direction, Displacement, Location, MathVec};
use std::default::default;

pub mod tools;
pub mod speed;

const JUMP_UPWARDS_MOTION: f64 = 0.42;

const SPRINT_SPEED: f64 = 0.2806;
const WALK_SPEED: f64 = 0.21585;
const SWIM_SPEED: f64 = 0.11;

const FALL_FACTOR: f64 = 0.02;

const FALL_TIMES: f64 = 0.9800000190734863;
const FALL_OFF_LAND: f64 = 0.5;

const LIQUID_MOTION_Y: f64 = 0.095;

const JUMP_WATER: f64 = 0.03999999910593033;
const ACC_G: f64 = 0.08;

const WATER_DECEL: f64 = 0.2;

const DRAG_MULT: f64 = 0.98; // 00000190734863;

// player width divided by 2
const PLAYER_WIDTH_2: f64 = 0.6 / 2.0;

// remove 0.1
const PLAYER_HEIGHT: f64 = 1.79999;

const UNIT_Y: Displacement = Displacement::new(0., 1., 0.);
const EPSILON_Y: Displacement = Displacement::new(0., 0.001, 0.);


#[derive(Debug)]
struct PendingMovement {
    strafe: Option<Strafe>,
    jump: bool,
    line: Option<Line>,
    speed: Speed,
}

impl Default for PendingMovement {
    fn default() -> Self {
        Self { strafe: None, line: None, speed: Speed::STOP, jump: false }
    }
}

fn effects_multiplier(speed: f64, slowness: f64) -> f64 {
    (1.0 + 0.2 * speed) * (1. - 0.15 * slowness)
}

// fn slipperiness_multiplier()

fn initial_ver(jump_boost: usize) -> f64 {
    0.42 + 0.1 * (jump_boost as f64)
}

fn ver_speed(prev_speed: f64) -> f64 {
    const GRAV: f64 = 0.08;
    const DRAG: f64 = 0.98;
    // if tentative_speed.abs() < 0.003 { 0. } else { tentative_speed }
    (prev_speed - GRAV) * DRAG
}

fn ground_speed(prev_speed: f64, prev_slip: f64, move_mult: f64, effect_mult: f64, slip: f64) -> f64 {
    let momentum = prev_speed * prev_slip * 0.91;
    let acc = 0.1 * move_mult * effect_mult * (0.6 / slip).pow(3);
    momentum + acc
}

fn jump_speed(prev_speed: f64, prev_slip: f64, move_mult: f64, effect_mult: f64, slip: f64, was_sprinting: bool) -> f64 {
    // let momentum = prev_speed * prev_slip * 0.91;
    // let acc = 0.1 * move_mult * effect_mult* (0.6 / slip).pow(3);
    let jump_sprint_boost = if was_sprinting { 0.2 } else { 0.0 };
    ground_speed(prev_speed, prev_slip, move_mult, effect_mult, slip) + jump_sprint_boost
}

fn air_speed(prev_speed: f64, prev_slip: f64, move_mult: f64) -> f64 {
    let momentum = prev_speed * prev_slip * 0.91;
    let acc = 0.02 * move_mult;
    momentum + acc
}


#[derive(Debug)]
struct MovementState {
    speeds: [f64; 2],
    slip: f64,
    falling: bool,
}

impl Default for MovementState{
    fn default() -> Self {
        MovementState {
            speeds: default(),
            slip: BlockKind::DEFAULT_SLIP,
            falling: false
        }
    }
}

/// # Purpose
/// Used to simulate a player position. Takes in movement events (such as jumping and strafing) and allows polling the resulting player data---for instance location.
/// # Resources
/// - [Minecaft Parkour](https://www.mcpk.wiki/wiki/Movement_Formulas)
#[derive(Debug, Default)]
pub struct Physics {
    location: Location,
    look: Direction,
    prev: MovementState,
    horizontal: Displacement,
    velocity: Displacement,
    pending: PendingMovement,
    in_water: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Strafe {
    Left,
    Right,
}

#[derive(Debug)]
pub enum Line {
    Forward,
    Backward,
}

pub fn mot_xz(mut strafe: f64, mut forward: f64, movement_factor: f64) -> [f64; 2] {
    let dist2 = strafe * strafe + forward * forward;
    if dist2 >= 1.0E-4 {
        let dist = dist2.sqrt().max(1.0);

        let multiplier = movement_factor / dist;

        strafe *= multiplier;
        forward *= multiplier;

        [strafe, forward]
    } else {
        [0., 0.]
    }
}

struct MovementProc {}

impl Physics {
    /// move to location and zero out velocity
    pub fn teleport(&mut self, location: Location) {
        self.location = location;
        self.velocity = Displacement::default();
    }

    pub fn jump(&mut self) {
        self.pending.jump = true;
    }

    pub fn look(&mut self, direction: Direction) {
        self.look = direction;
        self.horizontal = direction.horizontal().unit_vector();
    }

    pub fn direction(&self) -> Direction {
        self.look
    }

    pub fn line(&mut self, line: Line) {
        self.pending.line = Some(line)
    }

    pub fn strafe(&mut self, strafe: Strafe) {
        self.pending.strafe = Some(strafe)
    }

    pub fn speed(&mut self, speed: Speed) {
        self.pending.speed = speed;
    }

    pub fn cross_section_empty(&self, loc: Location, world: &WorldBlocks) -> bool {
        let dif_x = [-PLAYER_WIDTH_2, PLAYER_WIDTH_2];
        let dif_z = [-PLAYER_WIDTH_2, PLAYER_WIDTH_2];

        for dx in dif_x {
            for dz in dif_z {
                let test_loc = loc + Displacement::new(dx, 0., dz);
                let test_block_loc = BlockLocation::from(test_loc);
                let solid = matches!(world.get_block_simple(test_block_loc), Some(SimpleType::Solid));
                if solid {
                    return false;
                }
            }
        }
        true
    }

    pub fn tick(&mut self, world: &WorldBlocks) {
        let below_loc = self.location - EPSILON_Y;
        let falling = self.cross_section_empty(below_loc, world);
        let below_block_loc = BlockLocation::from(below_loc);

        let mut slip = match world.get_block(below_block_loc) {
            Some(BlockApprox::Realized(block)) => {
                block.kind().slip()
            }
            // we might be on the edge of a block
            _ => BlockKind::DEFAULT_SLIP
        };

        // the horizontal direction we are moving determined from the look direction
        let horizontal = self.horizontal;

        let [strafe_change, forward_change] = {
            let strafe_factor = match self.pending.strafe {
                None => 0.0,
                Some(Strafe::Right) => 1.0,
                Some(Strafe::Left) => -1.0,
            };

            let line_factor = match self.pending.line {
                None => 0.0,
                Some(Line::Forward) => 1.0,
                Some(Line::Backward) => -1.0,
            };

            let move_factor = self.pending.speed.multiplier();

            mot_xz(strafe_factor, line_factor, move_factor)
        };

        let sideway = horizontal.cross(UNIT_Y);

        let move_mults = [
            horizontal.dx * forward_change + sideway.dx * strafe_change,
            horizontal.dz * forward_change + sideway.dz * strafe_change,
        ];

        let effect_mult = effects_multiplier(0.0, 0.0);

        let mut speeds = [0.0, 0.0];

        let MovementState { speeds: prev_speeds, slip: prev_slip, .. } = self.prev;

        self.velocity.dy = if falling {
            // when falling the slip of air is 1.0
            slip = 1.0;
            for i in 0..2 {
                speeds[i] = air_speed(prev_speeds[i], prev_slip, move_mults[i])
            }
            ver_speed(self.velocity.dy)
        } else if self.pending.jump {

            for i in 0..2 {
                let is_sprinting = self.pending.speed == Speed::SPRINT;
                speeds[i] = jump_speed(prev_speeds[i], prev_slip, move_mults[i], effect_mult, slip, is_sprinting);
            }
            initial_ver(0)
        } else {

            // we are not falling and not jumping
            for i in 0..2 {
                speeds[i] = ground_speed(prev_speeds[i], prev_slip, move_mults[i], effect_mult, slip);
            }
            0.0
        };

        self.velocity.dx = speeds[0];
        self.velocity.dz = speeds[1];

        let prev_loc = self.location;

        {
            let dx = self.velocity.dx;
            let extra_dx = if dx == 0.0 { 0.0 } else { dx.signum() * PLAYER_WIDTH_2 };
            let end_dx = dx + extra_dx;

            let dz = self.velocity.dz;
            let extra_dz = if dz == 0.0 { 0.0 } else { dz.signum() * PLAYER_WIDTH_2 };
            let end_dz = dz + extra_dz;

            // let dy = self.velocity.dy;

            let end_vel = Displacement::new(end_dx, self.velocity.dy, end_dz);
            let new_loc = prev_loc + end_vel;

            let prev_legs: BlockLocation = prev_loc.into();
            let legs: BlockLocation = new_loc.into();
            let head: BlockLocation = {
                let mut head_loc = new_loc;
                head_loc.y += PLAYER_HEIGHT;
                head_loc.into()
            };

            let prev_legs_block = world.get_block_simple(prev_legs);
            let leg_block = world.get_block_simple(legs);
            let head_block = world.get_block_simple(head);


            let against_block = leg_block == Some(SimpleType::Solid) || head_block == Some(SimpleType::Solid);
            if against_block {
                // TODO: might break shit
                self.velocity.dx = 0.0;
                self.velocity.dz = 0.0;

                self.in_water = prev_legs_block == Some(SimpleType::Water) || head_block == Some(SimpleType::Water);
            } else {
                self.in_water = leg_block == Some(SimpleType::Water) || head_block == Some(SimpleType::Water);
            }
        }


        let mut new_loc = prev_loc + self.velocity;

        if falling {


            if self.velocity.dy >= 0.0 {// we are moving up

                let mut head_loc = new_loc + EPSILON_Y;
                head_loc.y += PLAYER_HEIGHT;

                if !self.cross_section_empty(head_loc, world) {
                    new_loc.y = head_loc.y.round() - PLAYER_HEIGHT - 0.0001;
                    self.velocity.dy = 0.0;
                }
            } else { // we are moving down
                if !self.cross_section_empty(new_loc - EPSILON_Y, world) {
                    new_loc.y = new_loc.y.round();
                    self.velocity.dy = 0.0;
                }
            }
        }

        self.location = new_loc;
        self.pending = PendingMovement::default();

        speeds[0] = self.velocity.dx;
        speeds[1] = self.velocity.dz;

        self.prev = MovementState {
            speeds,
            slip,
            falling
        }
    }

    pub fn on_ground(&self) -> bool {
        !self.prev.falling
    }
    pub fn location(&self) -> Location {
        self.location
    }


    pub fn velocity(&self) -> Displacement {
        self.velocity
    }
}
