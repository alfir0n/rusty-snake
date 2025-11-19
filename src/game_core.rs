use std::fmt;
use serde::{Deserialize, Serialize};

// Shared game constants
pub const GRID_WIDTH: i32 = 60;
pub const GRID_HEIGHT: i32 = 30;
// Client owns CELL_SIZE for rendering; server ticks use MOVE_INTERVAL_MS
pub const MOVE_INTERVAL_MS: u64 = 150; // ~6.67 FPS like original 0.15s

pub const MAX_PLAYERS: usize = 1;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Debug,Serialize, Deserialize)]
pub struct PlayerState {
    pub name: String,
    pub snake: Vec<Pos>,
    pub dir: Direction,
    pub score: u32,
    pub latest_input: Option<Direction>,
    pub dead: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            name: "".to_string(),
            snake: vec![Pos{ x: 0, y: 0}],
            dir: Default::default(),
            score: 0,
            latest_input: None,
            dead: false,
        }
    }
}

impl Default for Direction {
    fn default() -> Self { Direction::Right }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateMsg {
    pub tick: u64,
    pub players: Vec<PlayerState>,
    pub food: Pos,
    pub game_over: bool,
    pub winner: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    Join { name: String },
    Input { dir: Direction },
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dir_str = match self {
            Direction::Up => "Up",
            Direction::Down => "Down",
            Direction::Left => "Left",
            Direction::Right => "Right",
        };
        write!(f, "{}", dir_str)
    }
}


// Helpers shared by server for wrapping and stepping
pub fn step_head(mut head: Pos, dir: Direction) -> Pos {
    match dir {
        Direction::Up => head.y -= 1,
        Direction::Down => head.y += 1,
        Direction::Left => head.x -= 1,
        Direction::Right => head.x += 1,
    }
    if head.x < 0 { head.x = GRID_WIDTH - 1; }
    else if head.x >= GRID_WIDTH { head.x = 0; }
    if head.y < 0 { head.y = GRID_HEIGHT - 1; }
    else if head.y >= GRID_HEIGHT { head.y = 0; }
    head
}
