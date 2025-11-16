use macroquad::prelude::*;
use serde_json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

use snake::game_core::{ClientMsg, Direction, Pos, StateMsg, GRID_HEIGHT, GRID_WIDTH};

const CELL_SIZE: f32 = 20.0; // rendering only

fn draw_rect_at(pos: Pos, color: Color) {
    let x = pos.x as f32 * CELL_SIZE;
    let y = pos.y as f32 * CELL_SIZE;
    draw_rectangle(x, y, CELL_SIZE - 2.0, CELL_SIZE - 2.0, color);
}

fn start_networking() -> (mpsc::Sender<ClientMsg>, mpsc::Receiver<StateMsg>) {
    let (tx_ui_to_net, rx_ui_to_net) = mpsc::channel::<ClientMsg>();
    let (tx_net_to_ui, rx_net_to_ui) = mpsc::channel::<StateMsg>();

    thread::spawn(move || {
        // Connect to server
        let stream = match TcpStream::connect("127.0.0.1:4000") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to connect to server: {}", e);
                return;
            }
        };
        stream.set_nodelay(true).ok();
        let mut writer = stream.try_clone().expect("clone stream");
        let reader_stream = stream;

        // Send Join
        let join = serde_json::to_string(&ClientMsg::Join).unwrap();
        let _ = writeln!(writer, "{}", join);
        let _ = writer.flush();

        // Reader thread: receive states
        let tx_states = tx_net_to_ui.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(reader_stream);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // disconnected
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if trimmed.is_empty() { continue; }
                        if let Ok(state) = serde_json::from_str::<StateMsg>(trimmed) {
                            let _ = tx_states.send(state);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Writer loop: forward UI inputs to server
        loop {
            match rx_ui_to_net.recv() {
                Ok(msg) => {
                    if let Ok(json) = serde_json::to_string(&msg) {
                        if writeln!(writer, "{}", json).and_then(|_| writer.flush()).is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break, // UI dropped
            }
        }
    });

    (tx_ui_to_net, rx_net_to_ui)
}

#[macroquad::main("Snake (Client)")]
async fn main() {
    let screen_w = GRID_WIDTH as f32 * CELL_SIZE;
    let screen_h = GRID_HEIGHT as f32 * CELL_SIZE;
    request_new_screen_size(screen_w, screen_h);

    let (tx_input, rx_state) = start_networking();

    let mut latest_state: Option<StateMsg> = None;

    loop {
        // Input: send direction changes to server
        let mut dir_press: Option<Direction> = None;
        if is_key_pressed(KeyCode::Up) { dir_press = Some(Direction::Up); }
        if is_key_pressed(KeyCode::Down) { dir_press = Some(Direction::Down); }
        if is_key_pressed(KeyCode::Left) { dir_press = Some(Direction::Left); }
        if is_key_pressed(KeyCode::Right) { dir_press = Some(Direction::Right); }
        if is_key_pressed(KeyCode::W) { dir_press = Some(Direction::Up); }
        if is_key_pressed(KeyCode::S) { dir_press = Some(Direction::Down); }
        if is_key_pressed(KeyCode::A) { dir_press = Some(Direction::Left); }
        if is_key_pressed(KeyCode::D) { dir_press = Some(Direction::Right); }

        if let Some(d) = dir_press {
            let _ = tx_input.send(ClientMsg::Input { dir: d });
        }

        if is_key_pressed(KeyCode::Escape) { break; }

        // Drain any received states (keep only latest)
        while let Ok(state) = rx_state.try_recv() {
            latest_state = Some(state);
        }

        // Render
        clear_background(BLACK);

        if let Some(state) = &latest_state {
            // Snake 1
            for (i, s) in state.snake1.iter().enumerate() {
                draw_rect_at(*s, if i == 0 { BLUE } else { DARKBLUE });
            }
            // Snake 2
            for (i, s) in state.snake2.iter().enumerate() {
                draw_rect_at(*s, if i == 0 { GREEN } else { DARKGREEN });
            }
            // Food
            draw_rect_at(state.food, RED);

            // Scores
            draw_text(
                &format!("P1: {}  P2: {}  Tick: {}", state.score1, state.score2, state.tick),
                10.0,
                20.0,
                24.0,
                WHITE,
            );

            if state.game_over {
                let text = match state.winner {
                    Some(1) => "Game Over - Player 1 wins!",
                    Some(2) => "Game Over - Player 2 wins!",
                    None => "Game Over - Draw!",
                    _ => "Game Over",
                };
                let ts = measure_text(text, None, 30, 1.0);
                draw_text(text, (screen_w - ts.width) / 2.0, screen_h / 2.0, 30.0, YELLOW);
            }
        } else {
            let text = "Connecting to server...";
            let ts = measure_text(text, None, 30, 1.0);
            draw_text(text, (screen_w - ts.width) / 2.0, screen_h / 2.0, 30.0, YELLOW);
        }

        next_frame().await;
    }
}
