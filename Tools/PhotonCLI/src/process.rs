// SPDX-License-Identifier: GPL-3.0-only
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, anyhow};

const SAVED_TAIL_LINES: usize = 40;
static CHILD_PROCESS_GROUP: AtomicI32 = AtomicI32::new(0);
static BACKGROUND_PROCESS_GROUP: AtomicI32 = AtomicI32::new(0);
static INTERRUPT_REQUESTED: AtomicBool = AtomicBool::new(false);
static CTRL_C_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();

pub struct ProcessOutcome {
    pub code: i32,
    pub last_lines: VecDeque<String>,
}

#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

enum Event {
    Output(Stream, Vec<u8>),
    ReaderFinished,
    Exited(io::Result<ExitStatus>),
}

pub fn run_logged(
    command: &mut Command,
    log_path: &std::path::Path,
    verbose: bool,
    mut observe: impl FnMut(&str),
) -> Result<ProcessOutcome> {
    install_ctrl_c_handler()?;
    let parent = log_path.parent().context("log path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create log directory {}", parent.display()))?;
    let mut log =
        File::create(log_path).with_context(|| format!("failed to create log file {}", log_path.display()))?;

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    configure_process_group(command);
    let mut child = command.spawn().with_context(|| command_description(command))?;
    set_active_child(&child);

    let stdout = child.stdout.take().context("failed to capture child stdout")?;
    let stderr = child.stderr.take().context("failed to capture child stderr")?;
    let (sender, receiver) = mpsc::channel();
    spawn_reader(stdout, Stream::Stdout, sender.clone());
    spawn_reader(stderr, Stream::Stderr, sender.clone());
    thread::spawn(move || {
        let _ = sender.send(Event::Exited(child.wait()));
    });

    let mut readers_finished = 0;
    let mut status = None;
    let mut last_lines = VecDeque::with_capacity(SAVED_TAIL_LINES);
    while readers_finished < 2 || status.is_none() {
        let event = receiver
            .recv()
            .context("child process output channel closed unexpectedly")?;
        match event {
            Event::Output(stream, bytes) => {
                log.write_all(&bytes).context("failed to write build log")?;
                let line = String::from_utf8_lossy(&bytes);
                observe(line.trim_end_matches(['\r', '\n']));
                save_tail(&mut last_lines, &line);
                if verbose {
                    write_terminal(stream, &bytes)?;
                }
            }
            Event::ReaderFinished => readers_finished += 1,
            Event::Exited(result) => status = Some(result.context("failed to wait for child process")?),
        }
    }
    log.flush().context("failed to flush build log")?;
    CHILD_PROCESS_GROUP.store(0, Ordering::SeqCst);

    let status = status.context("child process exited without a status")?;
    Ok(ProcessOutcome {
        code: status_code(status),
        last_lines,
    })
}

pub fn run_inherited(command: &mut Command) -> Result<i32> {
    install_ctrl_c_handler()?;
    configure_process_group(command);
    let mut child = command.spawn().with_context(|| command_description(command))?;
    set_active_child(&child);
    let status = child.wait().context("failed to wait for child process")?;
    CHILD_PROCESS_GROUP.store(0, Ordering::SeqCst);
    Ok(status_code(status))
}

pub fn start_managed(command: &mut Command) -> Result<Child> {
    install_ctrl_c_handler()?;
    configure_process_group(command);
    let child = command.spawn().with_context(|| command_description(command))?;
    set_active_child(&child);
    Ok(child)
}

pub fn try_wait_managed(child: &mut Child) -> Result<Option<i32>> {
    let Some(status) = child.try_wait().context("failed to check Photon process")? else {
        return Ok(None);
    };
    clear_active_child(child);
    Ok(Some(status_code(status)))
}

pub fn stop_managed(child: &mut Child) -> Result<()> {
    if try_wait_managed(child)?.is_some() {
        return Ok(());
    }

    interrupt_managed(child);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if try_wait_managed(child)?.is_some() {
            return Ok(());
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }

    terminate_managed(child);
    let _ = child.wait();
    clear_active_child(child);
    Ok(())
}

pub fn interrupt_requested() -> bool {
    INTERRUPT_REQUESTED.load(Ordering::SeqCst)
}

pub fn start_background(command: &mut Command) -> Result<Child> {
    install_ctrl_c_handler()?;
    configure_process_group(command);
    let child = command.spawn().with_context(|| command_description(command))?;
    if let Ok(process_group) = i32::try_from(child.id()) {
        BACKGROUND_PROCESS_GROUP.store(process_group, Ordering::SeqCst);
    }
    Ok(child)
}

pub fn stop_background(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        BACKGROUND_PROCESS_GROUP.store(0, Ordering::SeqCst);
        return Ok(());
    }

    interrupt_background(child);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            BACKGROUND_PROCESS_GROUP.store(0, Ordering::SeqCst);
            return Ok(());
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }

    terminate_background(child);
    let _ = child.wait();
    BACKGROUND_PROCESS_GROUP.store(0, Ordering::SeqCst);
    Ok(())
}

fn spawn_reader(reader: impl Read + Send + 'static, stream: Stream, sender: mpsc::Sender<Event>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut bytes = Vec::new();
            let Ok(read) = reader.read_until(b'\n', &mut bytes) else {
                break;
            };
            if read == 0 {
                break;
            }
            if sender.send(Event::Output(stream, bytes)).is_err() {
                return;
            }
        }
        let _ = sender.send(Event::ReaderFinished);
    });
}

fn save_tail(tail: &mut VecDeque<String>, output: &str) {
    for line in output.lines() {
        if tail.len() == SAVED_TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line.to_owned());
    }
}

fn write_terminal(stream: Stream, bytes: &[u8]) -> Result<()> {
    match stream {
        Stream::Stdout => io::stdout().lock().write_all(bytes),
        Stream::Stderr => io::stderr().lock().write_all(bytes),
    }
    .context("failed to stream child output")
}

fn command_description(command: &Command) -> String {
    format!(
        "failed to start {:?} with arguments {:?}",
        command.get_program(),
        command.get_args().collect::<Vec<_>>()
    )
}

fn install_ctrl_c_handler() -> Result<()> {
    let result = CTRL_C_HANDLER.get_or_init(|| {
        ctrlc::set_handler(|| {
            INTERRUPT_REQUESTED.store(true, Ordering::SeqCst);
            let process_group = CHILD_PROCESS_GROUP.load(Ordering::SeqCst);
            if process_group > 0 {
                forward_interrupt(process_group);
            }
            let background_process_group = BACKGROUND_PROCESS_GROUP.load(Ordering::SeqCst);
            if background_process_group > 0 {
                forward_interrupt(background_process_group);
            }
        })
        .map_err(|error| error.to_string())
    });
    result.as_ref().map_err(|error| anyhow!(error.clone())).copied()
}

#[cfg(unix)]
fn interrupt_background(child: &Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGINT);
}

#[cfg(not(unix))]
fn interrupt_background(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(unix)]
fn terminate_background(child: &Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
}

#[cfg(not(unix))]
fn terminate_background(child: &mut Child) {
    let _ = child.kill();
}

fn set_active_child(child: &std::process::Child) {
    if let Ok(process_group) = i32::try_from(child.id()) {
        CHILD_PROCESS_GROUP.store(process_group, Ordering::SeqCst);
    }
}

fn clear_active_child(child: &std::process::Child) {
    if let Ok(process_group) = i32::try_from(child.id()) {
        let _ = CHILD_PROCESS_GROUP.compare_exchange(process_group, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

#[cfg(unix)]
fn interrupt_managed(child: &Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGINT);
}

#[cfg(not(unix))]
fn interrupt_managed(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(unix)]
fn terminate_managed(child: &Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
}

#[cfg(not(unix))]
fn terminate_managed(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn forward_interrupt(process_group: i32) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let _ = killpg(Pid::from_raw(process_group), Signal::SIGINT);
}

#[cfg(not(unix))]
fn forward_interrupt(_process_group: i32) {}

#[cfg(unix)]
fn status_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1)
}

#[cfg(not(unix))]
fn status_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_only_the_most_recent_lines() {
        let mut tail = VecDeque::new();
        for index in 0..=SAVED_TAIL_LINES {
            save_tail(&mut tail, &format!("line {index}\n"));
        }

        assert_eq!(tail.front().map(String::as_str), Some("line 1"));
    }
}
