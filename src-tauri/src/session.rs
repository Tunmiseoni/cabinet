use crate::launcher::{LaunchSpec, MatchConfig};
use serde::Serialize;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const MATCH_EVENT: &str = "match-state-changed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceState {
    pub side: u8,
    pub side_label: String,
    pub port: u16,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchState {
    pub status: String,
    pub rom: Option<String>,
    pub peer_ip: Option<String>,
    pub dev: bool,
    pub started_at_ms: Option<u64>,
    pub instances: Vec<InstanceState>,
    pub message: Option<String>,
}

impl Default for MatchState {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            rom: None,
            peer_ip: None,
            dev: false,
            started_at_ms: None,
            instances: Vec::new(),
            message: None,
        }
    }
}

pub struct Plan {
    pub spec: LaunchSpec,
    pub config: MatchConfig,
}

struct Running {
    child: Child,
    side: u8,
}

#[derive(Default)]
struct SessionInner {
    generation: u64,
    running: Vec<Running>,
    state: MatchState,
}

#[derive(Default)]
pub struct Session {
    inner: Mutex<SessionInner>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn emit(app: &AppHandle, state: &MatchState) {
    app.emit(MATCH_EVENT, state).ok();
}

fn spawn_child(plan: &Plan) -> Result<Child, String> {
    let mut command = Command::new(&plan.spec.program);
    command
        .args(&plan.spec.args)
        .current_dir(&plan.spec.cwd)
        .envs(plan.spec.envs.iter().cloned());
    command
        .spawn()
        .map_err(|err| format!("failed to launch {}: {err}", plan.spec.program.display()))
}

fn clear_running(inner: &mut SessionInner) {
    for mut running in inner.running.drain(..) {
        let _ = running.child.kill();
        let _ = running.child.wait();
    }
}

pub fn launch_many(
    app: &AppHandle,
    plans: &[Plan],
    dev: bool,
    peer_display: String,
) -> Result<MatchState, String> {
    if plans.is_empty() {
        return Err("nothing to launch".into());
    }

    let session = app.state::<Session>();
    let mut inner = session.inner.lock().unwrap();
    clear_running(&mut inner);
    inner.generation += 1;
    let generation = inner.generation;

    let mut running: Vec<Running> = Vec::new();
    let mut instances = Vec::new();

    for plan in plans {
        let child = match spawn_child(plan) {
            Ok(child) => child,
            Err(err) => {
                for mut started in running.drain(..) {
                    let _ = started.child.kill();
                    let _ = started.child.wait();
                }
                inner.state = MatchState::default();
                return Err(err);
            }
        };
        let pid = child.id();
        running.push(Running {
            child,
            side: plan.config.side,
        });
        instances.push(InstanceState {
            side: plan.config.side,
            side_label: plan.config.side_label().to_string(),
            port: plan.config.local_port(),
            pid: Some(pid),
            exit_code: None,
            message: None,
        });
    }

    let state = MatchState {
        status: "running".to_string(),
        rom: plans.first().map(|plan| plan.config.rom.clone()),
        peer_ip: Some(peer_display),
        dev,
        started_at_ms: Some(now_ms()),
        instances,
        message: None,
    };

    inner.running = running;
    inner.state = state.clone();
    drop(inner);

    emit(app, &state);
    spawn_monitor(app.clone(), generation);
    Ok(state)
}

pub fn launch(
    app: &AppHandle,
    spec: &LaunchSpec,
    config: &MatchConfig,
    dev: bool,
) -> Result<MatchState, String> {
    let plans = vec![Plan {
        spec: spec.clone(),
        config: config.clone(),
    }];
    launch_many(app, &plans, dev, config.peer_ip.clone())
}

fn spawn_monitor(app: AppHandle, generation: u64) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(300));

        let session = app.state::<Session>();
        let mut inner = session.inner.lock().unwrap();
        if inner.generation != generation || inner.running.is_empty() {
            break;
        }

        let running = std::mem::take(&mut inner.running);
        let mut still = Vec::new();
        let mut changed = false;

        for mut entry in running {
            match entry.child.try_wait() {
                Ok(Some(status)) => {
                    changed = true;
                    finish_slot(&mut inner.state, entry.side, status.code());
                }
                Ok(None) => still.push(entry),
                Err(_) => {
                    changed = true;
                    finish_slot(&mut inner.state, entry.side, None);
                }
            }
        }

        inner.running = still;
        let done = inner.running.is_empty();
        if done && inner.state.status == "running" {
            inner.state.status = "finished".to_string();
            inner.state.message = Some("all instances exited".to_string());
            changed = true;
        }

        let state = inner.state.clone();
        drop(inner);

        if changed {
            emit(&app, &state);
        }
        if done {
            break;
        }
    });
}

fn finish_slot(state: &mut MatchState, side: u8, code: Option<i32>) {
    if let Some(slot) = state
        .instances
        .iter_mut()
        .find(|slot| slot.side == side && slot.pid.is_some())
    {
        slot.pid = None;
        slot.exit_code = code;
        slot.message = Some(match code {
            Some(0) => "exited normally".to_string(),
            Some(code) => format!("exited with code {code}"),
            None => "terminated".to_string(),
        });
    }
}

pub fn stop(app: &AppHandle) -> Result<MatchState, String> {
    let session = app.state::<Session>();
    let mut inner = session.inner.lock().unwrap();
    inner.generation += 1;
    let had_running = !inner.running.is_empty();
    clear_running(&mut inner);

    if had_running {
        inner.state.status = "finished".to_string();
        inner.state.message = Some("stopped by user".to_string());
        for slot in inner.state.instances.iter_mut() {
            if slot.pid.is_some() {
                slot.pid = None;
                slot.message = Some("stopped by user".to_string());
            }
        }
    }

    let state = inner.state.clone();
    drop(inner);

    if had_running {
        emit(app, &state);
    }
    Ok(state)
}

pub fn status(app: &AppHandle) -> MatchState {
    app.state::<Session>().inner.lock().unwrap().state.clone()
}
