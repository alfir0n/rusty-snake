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

fn start_networking(server_addr: String, username: String) -> (mpsc::Sender<ClientMsg>, mpsc::Receiver<StateMsg>) {
    let (tx_ui_to_net, rx_ui_to_net) = mpsc::channel::<ClientMsg>();
    let (tx_net_to_ui, rx_net_to_ui) = mpsc::channel::<StateMsg>();

    thread::spawn(move || {
        // Connect to server
        let stream = match TcpStream::connect(&server_addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to connect to {}: {}", server_addr, e);
                return;
            }
        };
        stream.set_nodelay(true).ok();
        let mut writer = stream.try_clone().expect("clone stream");
        let reader_stream = stream;

        // Send Join with username
        let join = serde_json::to_string(&ClientMsg::Join { name: username }).unwrap();
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

#[derive(Copy, Clone, PartialEq, Eq)]
enum Focus { None, Name, Address }

fn draw_input_box(rect: Rect, text: &str, placeholder: &str, focused: bool) {
    // Box
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, if focused { YELLOW } else { GRAY });
    // Text
    let show = if text.is_empty() { placeholder } else { text };
    let color = if text.is_empty() { GRAY } else { WHITE };
    draw_text(show, rect.x + 8.0, rect.y + rect.h * 0.65, 28.0, color);
}

fn handle_text_input(current: &mut String) {
    // Typeable characters
    while let Some(c) = get_char_pressed() {
        // Filter out control chars except Enter handled elsewhere
        if !c.is_control() {
            current.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        current.pop();
    }
}

#[macroquad::main("Snake (Client)")]
async fn main() {
    let screen_w = GRID_WIDTH as f32 * CELL_SIZE;
    let screen_h = GRID_HEIGHT as f32 * CELL_SIZE;
    request_new_screen_size(screen_w, screen_h);

    // Connection UI state
    let mut username = String::new();
    let mut server_addr = String::from("127.0.0.1:4000");
    let mut focus = Focus::Name;
    let mut connected = false;

    // Networking channels (filled on connect)
    let mut tx_input_opt: Option<mpsc::Sender<ClientMsg>> = None;
    let mut rx_state_opt: Option<mpsc::Receiver<StateMsg>> = None;
    let mut latest_state: Option<StateMsg> = None;

    // Simple layout
    let panel_w = screen_w * 0.8;
    let panel_h = screen_h * 0.5;
    let panel_x = (screen_w - panel_w) * 0.5;
    let panel_y = (screen_h - panel_h) * 0.5;

    loop {
        clear_background(BLACK);

        if !connected {
            // Panel
            draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::new(0.1, 0.1, 0.1, 0.9));
            let title = "Multiplayer Snake";
            let ts = measure_text(title, None, 40, 1.0);
            draw_text(title, panel_x + (panel_w - ts.width) / 2.0, panel_y + 50.0, 40.0, WHITE);

            // Inputs
            let name_rect = Rect { x: panel_x + 40.0, y: panel_y + 90.0, w: panel_w - 80.0, h: 48.0 };
            let addr_rect = Rect { x: panel_x + 40.0, y: panel_y + 160.0, w: panel_w - 80.0, h: 48.0 };

            // Focus handling
            if is_mouse_button_pressed(MouseButton::Left) {
                let (mx, my) = mouse_position();
                let p = vec2(mx, my);
                if name_rect.contains(p) {
                    focus = Focus::Name;
                } else if addr_rect.contains(p) {
                    focus = Focus::Address;
                } else {
                    focus = Focus::None;
                }
            }

            // Input
            match focus {
                Focus::Name => handle_text_input(&mut username),
                Focus::Address => handle_text_input(&mut server_addr),
                Focus::None => {}
            }

            draw_input_box(name_rect, &username, "Username", focus == Focus::Name);
            draw_input_box(addr_rect, &server_addr, "Server address (e.g., 127.0.0.1:4000)", focus == Focus::Address);

            // Connect button
            let btn_rect = Rect { x: panel_x + panel_w - 200.0, y: panel_y + panel_h - 70.0, w: 160.0, h: 44.0 };
            let (mx, my) = mouse_position();
            let hovering = btn_rect.contains(vec2(mx, my));
            draw_rectangle(btn_rect.x, btn_rect.y, btn_rect.w, btn_rect.h, if hovering { DARKGREEN } else { GREEN });
            let btxt = "Connect";
            let bt = measure_text(btxt, None, 28, 1.0);
            draw_text(btxt, btn_rect.x + (btn_rect.w - bt.width) / 2.0, btn_rect.y + 32.0, 28.0, BLACK);

            let can_connect = !username.is_empty() && !server_addr.is_empty();
            if can_connect && (hovering && is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Enter)) {
                let (tx_input, rx_state) = start_networking(server_addr.clone(), username.clone());
                tx_input_opt = Some(tx_input);
                rx_state_opt = Some(rx_state);
                // Transition to game view; it will show "Connecting..." until a state arrives
                connected = true;
            }
        } else {
            // Game view
            // Input: send direction changes to server
            if let Some(tx_input) = &tx_input_opt {
                let mut dir_press: Option<Direction> = None;
                if is_key_pressed(KeyCode::Up) { dir_press = Some(Direction::Up); }
                if is_key_pressed(KeyCode::Down) { dir_press = Some(Direction::Down); }
                if is_key_pressed(KeyCode::Left) { dir_press = Some(Direction::Left); }
                if is_key_pressed(KeyCode::Right) { dir_press = Some(Direction::Right); }
                if is_key_pressed(KeyCode::W) { dir_press = Some(Direction::Up); }
                if is_key_pressed(KeyCode::S) { dir_press = Some(Direction::Down); }
                if is_key_pressed(KeyCode::A) { dir_press = Some(Direction::Left); }
                if is_key_pressed(KeyCode::D) { dir_press = Some(Direction::Right); }

                if let Some(d) = dir_press { let _ = tx_input.send(ClientMsg::Input { dir: d }); }
            }

            // Drain any received states (keep only latest)
            if let Some(rx_state) = &rx_state_opt {
                while let Ok(state) = rx_state.try_recv() {
                    latest_state = Some(state);
                }
            }

            // Render
            if let Some(state) = &latest_state {

                for p in state.players.iter() {
                    for (i, s) in p.snake.iter().enumerate() {
                        draw_rect_at(*s, if i == 0 { BLUE } else { DARKBLUE });
                    }
                }

                draw_rect_at(state.food, RED);

                let mut score_line = String::new();
                for p in state.players.iter() {
                    let  line= format!("{}: {}", p.name, p.score);
                    score_line += &line;
                }

                score_line += &format!("Ticks: {}", state.tick);

                draw_text(&score_line, 10.0, 20.0, 24.0, WHITE );

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

            // Optional: allow Esc to return to menu for reconnect
            if is_key_pressed(KeyCode::Escape) {
                connected = false;
                tx_input_opt = None;
                rx_state_opt = None;
                latest_state = None;
            }
        }

        next_frame().await;
    }
}