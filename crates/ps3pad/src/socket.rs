//! Newline-delimited JSON control socket for the daemon.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::daemon::{Command, Status};

pub fn start(path: &Path, status: Arc<Mutex<Status>>, commands: Sender<Command>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    if path.exists() {
        // Only remove stale sockets, never a regular file.
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_socket() {
            let _ = fs::remove_file(path);
        } else {
            anyhow::bail!("{} exists and is not a socket", path.display());
        }
    }

    let listener =
        UnixListener::bind(path).with_context(|| format!("cannot bind {}", path.display()))?;

    thread::Builder::new()
        .name("ps3pad-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let status = Arc::clone(&status);
                let commands = commands.clone();
                thread::spawn(move || {
                    if let Err(error) = handle(stream, status, commands) {
                        eprintln!("socket client error: {error:#}");
                    }
                });
            }
        })
        .context("cannot start socket thread")?;

    Ok(())
}

fn handle(stream: UnixStream, status: Arc<Mutex<Status>>, commands: Sender<Command>) -> Result<()> {
    let mut writer = stream.try_clone().context("cannot clone socket")?;
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        let line = line.context("cannot read socket command")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                respond(
                    &mut writer,
                    json!({"ok": false, "error": error.to_string()}),
                )?;
                continue;
            }
        };
        let command = request.get("cmd").and_then(Value::as_str).unwrap_or("");
        match command {
            "ping" => respond(&mut writer, json!({"ok": true, "pong": true}))?,
            "status" => {
                let status = status.lock().unwrap();
                respond(&mut writer, serde_json::to_value(&*status)?)?;
            }
            "stop" => {
                let _ = commands.send(Command::Stop);
                respond(&mut writer, json!({"ok": true, "stopping": true}))?;
            }
            "led" => {
                let player = request.get("player").and_then(Value::as_u64).unwrap_or(1);
                if !(1..=4).contains(&player) {
                    respond(
                        &mut writer,
                        json!({"ok": false, "error": "player must be 1..=4"}),
                    )?;
                    continue;
                }
                let _ = commands.send(Command::Led(player as u8));
                respond(&mut writer, json!({"ok": true, "player": player}))?;
            }
            "rumble" => {
                let left = request
                    .get("left")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .min(255) as u8;
                let right = request
                    .get("right")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let _ = commands.send(Command::Rumble { left, right });
                respond(
                    &mut writer,
                    json!({"ok": true, "left": left, "right": right}),
                )?;
            }
            other => {
                respond(
                    &mut writer,
                    json!({
                        "ok": false,
                        "error": format!("unknown command {other:?}"),
                        "commands": ["ping", "status", "stop", "led", "rumble"],
                    }),
                )?;
            }
        }
    }
    Ok(())
}

fn respond(writer: &mut UnixStream, value: Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, &value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
