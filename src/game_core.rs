use serde::{Deserialize, Serialize};

// Shared game constants
pub const GRID_WIDTH: i32 = 60;
pub const GRID_HEIGHT: i32 = 30;
// Client owns CELL_SIZE for rendering; server ticks use MOVE_INTERVAL_MS
pub const MOVE_INTERVAL_MS: u64 = 150; // ~6.67 FPS like original 0.15s

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

impl Default for Direction {
    fn default() -> Self { Direction::Right }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateMsg {
    pub tick: u64,
    pub snake1: Vec<Pos>,
    pub snake2: Vec<Pos>,
    pub dir1: Direction,
    pub dir2: Direction,
    pub food: Pos,
    pub score1: u32,
    pub score2: u32,
    pub game_over: bool,
    pub winner: Option<u8>, // 1 or 2; None for draw
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    Join,
    Input { dir: Direction },
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
