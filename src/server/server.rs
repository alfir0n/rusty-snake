use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc};
use std::thread;
use std::time::{Duration, Instant};

use rand::Rng;
use serde_json;
use snake::game_core::{ClientMsg, Direction, GRID_HEIGHT, GRID_WIDTH, MOVE_INTERVAL_MS, Pos, StateMsg, step_head, PlayerState, MAX_PLAYERS};



struct ServerState {
    tick: u64,
    players: Vec<PlayerState>,
    food: Pos,
    game_over: bool,
    winner: Option<u8>,
}

impl ServerState {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let mut s = Self {
            tick: 0,
            players: vec![PlayerState::default(); MAX_PLAYERS],
            food: Pos {
                x: rng.gen_range(0..GRID_WIDTH),
                y: rng.gen_range(0..GRID_HEIGHT),
            },
            game_over: false,
            winner: None,
        };
        s.respawn_food();
        s
    }

    fn contains_any(&self, pos: &Pos) -> bool {
        for player in self.players.iter() {
            if player.snake.contains(pos) {
                true;
            }
        }
        false
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

        for player in self.players.iter_mut() {
            // prevent 180 deg turn
            if let Some(dir) = player.latest_input.take() {
                let opposite = match player.dir {
                    Direction::Up => Direction::Down,
                    Direction::Down => Direction::Up,
                    Direction::Left => Direction::Right,
                    Direction::Right => Direction::Left,
                };

                if dir != opposite {
                    player.dir = dir;
                }
            }
        }
    }

    fn step(&mut self) {
        if self.game_over {
            return;
        }

        self.tick += 1;
        self.apply_inputs();

        // calculate new positions
        let mut new_positions = [Pos::default(); MAX_PLAYERS];
        for (i, player) in self.players.iter_mut().enumerate() {
            let snake_head = *player.snake.first().unwrap();
            new_positions[i] = step_head( snake_head, player.dir);

        }

        // detect collisions and derive player status
        let mut player_status = [false; MAX_PLAYERS];
        for (i, pos) in new_positions.iter().enumerate() {
            for player in self.players.iter() {
                if !player.dead{
                    player_status[i] = player.snake.contains(pos);
                }
            }
        }
        //update player status
        for (i, status) in player_status.iter().enumerate() {
            self.players[i].dead = *status;
        }

        // check if and which player grabs food
        let mut player_grabbed_food = None;
        for (i, pos) in new_positions.iter().enumerate() {
            if self.food == *pos && !self.players[i].dead {
                player_grabbed_food = Some(i);
            }
        }


        //process next steps for player's snake
        for (i, pos) in new_positions.iter().enumerate() {
            if !self.players[i].dead {
                self.players[i].snake.insert(0, *pos);
                if player_grabbed_food != None && player_grabbed_food.unwrap() == i {
                    self.respawn_food();
                    self.players[i].score += 1;
                }
                else { self.players[i].snake.pop(); }
            }
        }


    }

    fn snapshot(&self) -> StateMsg {
        StateMsg {
            tick: self.tick,
            players: self.players.clone(),
            food: self.food,
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
    for player_id in 1..=MAX_PLAYERS as u8 {
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
                ClientMsg::Join { name } => {
                    state.players[pid as usize - 1].name = name.clone();
                    println!("Welcome {}!", name );
                }
                // If
                ClientMsg::Input { dir } => {
                    state.players[pid as usize - 1].latest_input = Some(dir);
                    println!("{} : {}", state.players[pid as usize - 1].name, dir.to_string() )
                }
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
