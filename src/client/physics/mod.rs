use std::lazy::SyncLazy;

use crate::storage::block::{BlockLocation, SimpleType};
use crate::storage::blocks::WorldBlocks;
use crate::types::{Direction, Displacement, Location};

pub mod tools;

const JUMP_UPWARDS_MOTION: f64 = 0.42;

const SPRINT_SPEED: f64 = 0.2806;
const WALK_SPEED: f64 = 0.21585;
const SWIM_SPEED: f64 = 0.11;

const FALL_FACTOR: f64 = 0.02;

const FALL_TIMES: f64 = 0.9800000190734863;
const FALL_OFF_LAND: f64 = 0.5;

const LIQUID_MOTION_Y: f64 = 0.095;

fn jump_factor(jump_boost: Option<u32>) -> f64 {
    JUMP_UPWARDS_MOTION + match jump_boost {
        None => 0.0,
        Some(level) => (level as f64 + 1.0) * 0.1
    }
}

const JUMP_WATER: f64 = 0.03999999910593033;
const ACC_G: f64 = 0.08;

const WATER_DECEL: f64 = 0.2;

const DRAG_MULT: f64 = 0.98; // 00000190734863;

// player width divided by 2
const PLAYER_WIDTH_2: f64 = 0.6 / 2.0;

// remove 0.1
const PLAYER_HEIGHT: f64 = 1.79;


// 1/2 at^2 + vt = 0
// t(1/2 at + v) = 0
// 1/2 at + v = 0
// t = -2v / a
// const JUMP_SECS: f64 = {
//     2.0 * jump_factor(None) / ACC_G
// };

/// Takes in normal Minecraft controls and tracks/records information
#[derive(Default, Debug)]
pub struct Physics {
    location: Location,
    look: Direction,
    horizontal: Displacement,
    velocity: Displacement,
    on_ground: bool,
    just_jumped: bool,
    just_descended: bool,
    pub in_water: bool,
}

pub enum Strafe {
    Left,
    Right,
}

pub enum Walk {
    Forward,
    Backward,
}

static UNIT_Y: SyncLazy<Displacement> = SyncLazy::new(|| {
    Displacement::new(0.0, 1.0, 0.0)
});

struct MovementProc {}

impl Physics {
    /// move to location and zero out velocity
    pub fn teleport(&mut self, location: Location) {
        self.location = location;
        self.velocity = Displacement::default();
    }

    pub fn jump(&mut self) {
        self.just_jumped = true;
    }

    pub fn descend(&mut self) {
        self.just_descended = true;
    }

    pub fn look(&mut self, direction: Direction) {
        self.look = direction;
        self.horizontal = direction.horizontal().unit_vector();
    }

    pub fn direction(&self) -> Direction {
        self.look
    }

    fn speed(&self) -> f64 {
        if self.in_water {
            SWIM_SPEED
        } else if self.on_ground {
            WALK_SPEED
        } else {
            0.05
        }
    }

    pub fn walk(&mut self, walk: Walk) {
        let mut velocity = self.horizontal;
        velocity *= self.speed();
        if let Walk::Backward = walk {
            velocity *= -1.0;
        }

        self.velocity.dx = velocity.dx;
        self.velocity.dz = velocity.dz;
    }

    pub fn strafe(&mut self, strafe: Strafe) {
        let mut velocity = self.horizontal.cross(*UNIT_Y);


        velocity *= self.speed();
        if let Strafe::Left = strafe {
            velocity *= -1.0
        }

        self.velocity.dx = velocity.dx;
        self.velocity.dz = velocity.dz;
    }

    pub fn is_falling(&self, world: &WorldBlocks) -> bool {
        let mut below_loc = self.location;
        below_loc.y -= 0.001;

        let dif_x = [-PLAYER_WIDTH_2, PLAYER_WIDTH_2];
        let dif_z = [-PLAYER_WIDTH_2, PLAYER_WIDTH_2];

        for dx in dif_x {
            for dz in dif_z {
                let test_loc = below_loc + Displacement::new(dx, 0., dz);
                let below_loc: BlockLocation = test_loc.into();
                let falling = matches!(world.get_block_simple(below_loc), Some(SimpleType::WalkThrough) | Some(SimpleType::Water));
                if !falling {
                    return false;
                }
            }
        }
        true
    }

    pub fn tick(&mut self, world: &WorldBlocks) {
        let jump = self.just_jumped;

        self.velocity.dy = if jump && self.on_ground {
            self.on_ground = false;
            jump_factor(None)
        } else if !self.on_ground {
            (self.velocity.dy - ACC_G) * DRAG_MULT
        } else {
            0.0
        };

        self.just_jumped = false;

        // move y, x, z
        let prev_loc = self.location;

        let falling = self.is_falling(world);

        if falling { self.on_ground = false; }

        // if falling && self.in_water && self.just_descended {
        //     self.just_descended = false;
        //     self.velocity.dy = (self.velocity.dy + WATER_DECEL).min(-LIQUID_MOTION_Y);
        // }

        {
            let dx = self.velocity.dx;
            let extra_dx = if dx == 0.0 { 0.0 } else { dx.signum() * PLAYER_WIDTH_2 };
            let end_dx = dx + extra_dx;

            let dz = self.velocity.dz;
            let extra_dz = if dz == 0.0 { 0.0 } else { dz.signum() * PLAYER_WIDTH_2 };
            let end_dz = dz + extra_dz;

            let dy = self.velocity.dy;

            let end_vel = Displacement::new(end_dx, dy, end_dz);
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
                let new_dx = 0.0;
                let new_dz = 0.0;
                self.velocity.dx = new_dx;
                self.velocity.dz = new_dz;

                // println!("against block ... {} => {}", prev_loc, new_loc);

                self.in_water = prev_legs_block == Some(SimpleType::Water) || head_block == Some(SimpleType::Water);
            } else {
                self.in_water = leg_block == Some(SimpleType::Water) || head_block == Some(SimpleType::Water);
            }
        }

        let mut new_loc = prev_loc + self.velocity;

        if !self.on_ground {
            let prev_block_loc: BlockLocation = prev_loc.into();
            let next_block_loc: BlockLocation = new_loc.into();

            let mut head_loc = new_loc;
            head_loc.y += PLAYER_HEIGHT;

            let head_loc = new_loc.into();

                if self.velocity.dy >= 0.0 {// we are moving up
                    if world.get_block_simple(head_loc) == Some(SimpleType::Solid) {
                        // we hit our heads!
                        self.velocity.dy = 0.0;
                        new_loc.y = (head_loc.y as f64) - PLAYER_HEIGHT;
                    } else if !self.in_water {
                        // we can decelerate normally
                        // self.velocity.dy -= ACC_G;
                    }
                } else { // we are moving down
                    match world.get_block_simple(next_block_loc) {
                        Some(SimpleType::Solid) => {
                            new_loc = prev_block_loc.center_bottom();
                            self.velocity.dy = 0.0;
                            self.on_ground = true;
                        }
                        // we are falling
                        Some(SimpleType::WalkThrough) => {
                            // self.velocity.dy -= ACC_G;
                        }
                        Some(SimpleType::Water) => {
                            // println!("water");
                            // self.velocity.dy -= ACC_G;
                        }
                        // the chunk hasn't loaded, let's not apply physics
                        _ => {}
                    }
                }
        }

        self.location = new_loc;

        // reset walk
        self.velocity.dx = 0.0;
        self.velocity.dz = 0.0;
    }
    pub fn location(&self) -> Location {
        self.location
    }


    pub fn velocity(&self) -> Displacement {
        self.velocity
    }
    pub fn on_ground(&self) -> bool {
        self.on_ground
    }
}
