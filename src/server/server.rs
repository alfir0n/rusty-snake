use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use rand::Rng;
use serde_json;

use snake::game_core::{
    ClientMsg, Direction, GRID_HEIGHT, GRID_WIDTH, MOVE_INTERVAL_MS, Pos, StateMsg, step_head,
};

#[derive(Default)]
struct PlayerState {
    snake: Vec<Pos>,
    dir: Direction,
    latest_input: Option<Direction>,
}

struct ServerState {
    tick: u64,
    p1: PlayerState,
    p2: PlayerState,
    food: Pos,
    score1: u32,
    score2: u32,
    game_over: bool,
    winner: Option<u8>,
}

impl ServerState {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let start1 = Pos {
            x: GRID_WIDTH / 2 - 3,
            y: GRID_HEIGHT / 2,
        };
        let start2 = Pos {
            x: GRID_WIDTH / 2 + 3,
            y: GRID_HEIGHT / 2,
        };
        let mut s = Self {
            tick: 0,
            p1: PlayerState {
                snake: vec![start1],
                dir: Direction::Right,
                latest_input: None,
            },
            p2: PlayerState {
                snake: vec![start2],
                dir: Direction::Left,
                latest_input: None,
            },
            food: Pos {
                x: rng.gen_range(0..GRID_WIDTH),
                y: rng.gen_range(0..GRID_HEIGHT),
            },
            score1: 0,
            score2: 0,
            game_over: false,
            winner: None,
        };
        s.respawn_food();
        s
    }

    fn contains_any(&self, pos: &Pos) -> bool {
        self.p1.snake.contains(pos) || self.p2.snake.contains(pos)
    }

    fn respawn_food(&mut self) {
        let mut rng = rand::thread_rng();
        loop {
            let pos = Pos {
                x: rng.gen_range(0..GRID_WIDTH),
                y: rng.gen_range(0..GRID_HEIGHT),
            };
            if !self.contains_any(&pos) {
                self.food = pos;
                break;
            }
        }
    }

    fn apply_inputs(&mut self) {
        // prevent 180° turns
        if let Some(d) = self.p1.latest_input.take() {
            let opposite = match self.p1.dir {
                Direction::Up => Direction::Down,
                Direction::Down => Direction::Up,
                Direction::Left => Direction::Right,
                Direction::Right => Direction::Left,
            };
            if d != opposite {
                self.p1.dir = d;
            }
        }
        if let Some(d) = self.p2.latest_input.take() {
            let opposite = match self.p2.dir {
                Direction::Up => Direction::Down,
                Direction::Down => Direction::Up,
                Direction::Left => Direction::Right,
                Direction::Right => Direction::Left,
            };
            if d != opposite {
                self.p2.dir = d;
            }
        }
    }

    fn step(&mut self) {
        if self.game_over {
            return;
        }
        self.tick += 1;
        self.apply_inputs();

        let h1 = step_head(*self.p1.snake.first().unwrap(), self.p1.dir);
        let h2 = step_head(*self.p2.snake.first().unwrap(), self.p2.dir);

        // collisions
        let p1_hits_self = self.p1.snake.contains(&h1);
        let p2_hits_self = self.p2.snake.contains(&h2);
        let p1_hits_p2 = self.p2.snake.contains(&h1);
        let p2_hits_p1 = self.p1.snake.contains(&h2);

        if (p1_hits_self || p1_hits_p2) && (p2_hits_self || p2_hits_p1) {
            self.game_over = true;
            self.winner = None;
            return;
        }
        if p1_hits_self || p1_hits_p2 {
            self.game_over = true;
            self.winner = Some(2);
            return;
        }
        if p2_hits_self || p2_hits_p1 {
            self.game_over = true;
            self.winner = Some(1);
            return;
        }

        let p1_eats = h1 == self.food;
        let p2_eats = h2 == self.food;

        self.p1.snake.insert(0, h1);
        self.p2.snake.insert(0, h2);

        match (p1_eats, p2_eats) {
            (true, false) => {
                self.score1 += 1;
                self.respawn_food();
                self.p2.snake.pop();
            }
            (false, true) => {
                self.score2 += 1;
                self.respawn_food();
                self.p1.snake.pop();

            }
            (true, true) => {
                self.score1 += 1;
                self.score2 += 1;
                self.respawn_food();
            }
            (false, false) => {
                self.p1.snake.pop();
                self.p2.snake.pop();
            }
        }
    }

    fn snapshot(&self) -> StateMsg {
        StateMsg {
            tick: self.tick,
            snake1: self.p1.snake.clone(),
            snake2: self.p2.snake.clone(),
            dir1: self.p1.dir,
            dir2: self.p2.dir,
            food: self.food,
            score1: self.score1,
            score2: self.score2,
            game_over: self.game_over,
            winner: self.winner,
        }
    }
}

fn spawn_reader(stream: TcpStream, player_slot: u8, tx_inputs: mpsc::Sender<(u8, ClientMsg)>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // disconnect
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(msg) = serde_json::from_str::<ClientMsg>(trimmed) {
                        let _ = tx_inputs.send((player_slot, msg));
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4000")?;
    println!("Server listening on 127.0.0.1:4000");

    let (tx_inputs, rx_inputs) = mpsc::channel::<(u8, ClientMsg)>();

    // Accept up to two clients
    let mut writers: Vec<(u8, TcpStream)> = Vec::new();
    for player_id in 1u8..=2u8 {
        let (stream, addr) = listener.accept()?;
        println!("Client connected: {} as Player {}", addr, player_id);
        stream.set_nodelay(true).ok();
        let reader_stream = stream.try_clone()?;
        spawn_reader(reader_stream, player_id, tx_inputs.clone());

        // Send a greeting or expect Join from client; we will just wait for Join but it's optional
        writers.push((player_id, stream));
    }

    // Initialize state
    let mut state = ServerState::new();

    // Simple input buffer; not strictly necessary
    let tick_duration = Duration::from_millis(MOVE_INTERVAL_MS);
    let mut next_tick = Instant::now() + tick_duration;

    loop {
        // handle any pending inputs (non-blocking)
        while let Ok((pid, msg)) = rx_inputs.try_recv() {
            match msg {
                ClientMsg::Join => {
                    // nothing required, player assigned on connect
                }
                ClientMsg::Input { dir } => match pid {
                    1 => state.p1.latest_input = Some(dir),
                    2 => state.p2.latest_input = Some(dir),
                    _ => {}
                },
            }
        }

        let now = Instant::now();
        if now >= next_tick {
            state.step();
            // broadcast
            let snapshot = state.snapshot();
            let json = serde_json::to_string(&snapshot).unwrap();
            writers.retain_mut(|(_pid, w)| {
                if writeln!(w, "{}", json).and_then(|_| w.flush()).is_err() {
                    // drop disconnected writer
                    false
                } else {
                    true
                }
            });
            next_tick += tick_duration;
        } else {
            thread::sleep(Duration::from_millis(1));
        }

        // End server when both clients disconnect
        if writers.is_empty() {
            break;
        }
    }

    println!("Server shutting down.");
    Ok(())
}
