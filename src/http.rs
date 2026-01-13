use core::fmt::Write;
use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use heapless::String;

use crate::stepper::Direction;

pub static MOTOR_COMMAND_CHANNEL: Channel<CriticalSectionRawMutex, MotorCommand, 4> =
    Channel::new();

#[derive(Debug, Clone, Copy)]
pub enum MotorCommand {
    Rotate { degrees: f32, direction: Direction },
    SetSpeed { rpm: f32 },
    Stop,
    Continuous { direction: Direction },
}

pub fn command_sender() -> Sender<'static, CriticalSectionRawMutex, MotorCommand, 4> {
    MOTOR_COMMAND_CHANNEL.sender()
}

const INDEX_HTML: &str = include_str!("web/index.html");

#[embassy_executor::task]
pub async fn http_server_task(stack: Stack<'static>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(30)));

        defmt::info!("Waiting for HTTP connection on port 80...");
        if let Err(e) = socket.accept(80).await {
            defmt::error!("Accept error: {:?}", e);
            continue;
        }
        defmt::info!("Client connected");

        let mut buf = [0; 1024];
        match socket.read(&mut buf).await {
            Ok(0) => continue,
            Ok(n) => {
                let request = core::str::from_utf8(&buf[..n]).unwrap_or("");
                handle_request(&mut socket, request).await;
            }
            Err(e) => {
                defmt::error!("Read error: {:?}", e);
            }
        }

        socket.close();
        embassy_time::Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
}

async fn handle_request(socket: &mut TcpSocket<'_>, request: &str) {
    let first_line = request.lines().next().unwrap_or("");
    let parts: heapless::Vec<&str, 8> = first_line.split_whitespace().collect();

    if parts.len() < 2 {
        send_response(socket, 400, "text/plain", "Bad Request").await;
        return;
    }

    let method = parts[0];
    let path = parts[1];

    match (method, path) {
        ("GET", "/") => {
            send_response(socket, 200, "text/html", INDEX_HTML).await;
        }
        ("POST", p) if p.starts_with("/api/") => {
            handle_api(socket, p, request).await;
        }
        _ => {
            send_response(socket, 404, "text/plain", "Not Found").await;
        }
    }
}

async fn handle_api(socket: &mut TcpSocket<'_>, path: &str, request: &str) {
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let sender = command_sender();

    match path {
        "/api/rotate" => {
            if let Some((degrees, dir)) = parse_rotate_body(body) {
                let _ = sender.try_send(MotorCommand::Rotate {
                    degrees,
                    direction: dir,
                });
                send_json_ok(socket).await;
            } else {
                send_response(
                    socket,
                    400,
                    "application/json",
                    r#"{"error":"invalid params"}"#,
                )
                .await;
            }
        }
        "/api/speed" => {
            if let Some(rpm) = parse_speed_body(body) {
                let _ = sender.try_send(MotorCommand::SetSpeed { rpm });
                send_json_ok(socket).await;
            } else {
                send_response(
                    socket,
                    400,
                    "application/json",
                    r#"{"error":"invalid rpm"}"#,
                )
                .await;
            }
        }
        "/api/continuous" => {
            if let Some(dir) = parse_direction_body(body) {
                let _ = sender.try_send(MotorCommand::Continuous { direction: dir });
                send_json_ok(socket).await;
            } else {
                send_response(
                    socket,
                    400,
                    "application/json",
                    r#"{"error":"invalid direction"}"#,
                )
                .await;
            }
        }
        "/api/stop" => {
            let _ = sender.try_send(MotorCommand::Stop);
            send_json_ok(socket).await;
        }
        _ => {
            send_response(socket, 404, "application/json", r#"{"error":"not found"}"#).await;
        }
    }
}

fn parse_rotate_body(body: &str) -> Option<(f32, Direction)> {
    let degrees = extract_json_number(body, "degrees")?;
    let dir = extract_json_string(body, "direction")?;
    let direction = match dir {
        "cw" => Direction::Clockwise,
        "ccw" => Direction::CounterClockwise,
        _ => return None,
    };
    Some((degrees, direction))
}

fn parse_speed_body(body: &str) -> Option<f32> {
    extract_json_number(body, "rpm")
}

fn parse_direction_body(body: &str) -> Option<Direction> {
    let dir = extract_json_string(body, "direction")?;
    match dir {
        "cw" => Some(Direction::Clockwise),
        "ccw" => Some(Direction::CounterClockwise),
        _ => None,
    }
}

fn extract_json_string<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pattern: String<64> = {
        let mut s = String::new();
        let _ = write!(s, "\"{}\":\"", key);
        s
    };
    let start = json.find(pattern.as_str())? + pattern.len();
    let end = json[start..].find('"')? + start;
    Some(&json[start..end])
}

fn extract_json_number(json: &str, key: &str) -> Option<f32> {
    let pattern: String<64> = {
        let mut s = String::new();
        let _ = write!(s, "\"{}\":", key);
        s
    };
    let start = json.find(pattern.as_str())? + pattern.len();
    let rest = json[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

async fn send_response(socket: &mut TcpSocket<'_>, status: u16, content_type: &str, body: &str) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Unknown",
    };

    let mut response: String<256> = String::new();
    let _ = write!(
        response,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        status_text,
        content_type,
        body.len()
    );

    write_all(socket, response.as_bytes()).await;
    write_all(socket, body.as_bytes()).await;
}

async fn write_all(socket: &mut TcpSocket<'_>, mut buf: &[u8]) {
    while !buf.is_empty() {
        match socket.write(buf).await {
            Ok(0) => break,
            Ok(n) => buf = &buf[n..],
            Err(_) => break,
        }
    }
}

async fn send_json_ok(socket: &mut TcpSocket<'_>) {
    send_response(socket, 200, "application/json", r#"{"status":"ok"}"#).await;
}
