use crate::config::{self, Config};
use crate::launcher::{LaunchSpec, MatchConfig};
use crate::provider::Role;
use crate::results::{self, MatchResult};
use crate::scores::{ScoreBoard, ScoreCounter, SCORES_EVENT};
use crate::tailscale::{self, PeerHealth};
use serde::Serialize;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const MATCH_EVENT: &str = "match-state-changed";
const HEALTH_INTERVAL: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceState {
    pub role: Role,
    pub role_label: String,
    pub port: Option<u16>,
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
    pub result: Option<MatchResult>,
    pub peer_health: Option<PeerHealth>,
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
            result: None,
            peer_health: None,
            message: None,
        }
    }
}

pub struct Plan {
    pub spec: LaunchSpec,
    pub role: Role,
    pub rom: String,
    pub peer_ip: String,
    pub port: Option<u16>,
    pub side: Option<u8>,
}

impl Plan {
    pub fn from_fightcade(spec: LaunchSpec, config: MatchConfig) -> Self {
        let role = if config.side == 0 { Role::P1 } else { Role::P2 };
        let port = Some(config.local_port());
        Self {
            spec,
            role,
            rom: config.rom,
            peer_ip: config.peer_ip,
            port,
            side: Some(config.side),
        }
    }
}

struct Running {
    child: Child,
    role: Role,
}

#[derive(Default)]
struct SessionInner {
    generation: u64,
    running: Vec<Running>,
    state: MatchState,
    overlay_dir: Option<PathBuf>,
    last_result: MatchResult,
    score_counter: Option<ScoreCounter>,
    score_opponent: Option<String>,
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
    let mut command = crate::process::command(&plan.spec.program);
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
    overlay: bool,
    track_scores: bool,
    peer_display: String,
) -> Result<MatchState, String> {
    if plans.is_empty() {
        return Err("nothing to launch".into());
    }
    let overlay = overlay && !dev;

    let session = app.state::<Session>();
    let mut inner = session.inner.lock().unwrap();
    clear_running(&mut inner);
    inner.generation += 1;
    let generation = inner.generation;

    let overlay_dir = if overlay {
        plans.first().map(|plan| results::overlay_dir(&plan.spec.cwd))
    } else {
        None
    };
    if let Some(dir) = &overlay_dir {
        results::reset(dir);
    }

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
            role: plan.role,
        });
        instances.push(InstanceState {
            role: plan.role,
            role_label: plan.role.label().to_string(),
            port: plan.port,
            pid: Some(pid),
            exit_code: None,
            message: None,
        });
    }

    let state = MatchState {
        status: "running".to_string(),
        rom: plans.first().map(|plan| plan.rom.clone()),
        peer_ip: Some(peer_display),
        dev,
        started_at_ms: Some(now_ms()),
        instances,
        result: None,
        peer_health: None,
        message: None,
    };

    inner.running = running;
    inner.overlay_dir = overlay_dir;
    inner.last_result = MatchResult::default();
    if overlay && track_scores && plans[0].side.is_some() {
        inner.score_counter = Some(ScoreCounter::new(plans[0].side.unwrap()));
        inner.score_opponent = Some(plans[0].peer_ip.clone());
    } else {
        inner.score_counter = None;
        inner.score_opponent = None;
    }
    inner.state = state.clone();
    drop(inner);

    emit(app, &state);
    spawn_monitor(app.clone(), generation);
    if !dev {
        spawn_health(app.clone(), generation, plans[0].peer_ip.clone());
    }
    Ok(state)
}

pub fn launch(
    app: &AppHandle,
    plan: &Plan,
    dev: bool,
    overlay: bool,
    track_scores: bool,
    peer_display: String,
) -> Result<MatchState, String> {
    launch_many(
        app,
        std::slice::from_ref(plan),
        dev,
        overlay,
        track_scores,
        peer_display,
    )
}

fn spawn_health(app: AppHandle, generation: u64, ip: String) {
    std::thread::spawn(move || {
        let binary = {
            let cfg = app
                .path()
                .app_config_dir()
                .map(|dir| Config::load(&config::config_path(dir)))
                .unwrap_or_default();
            tailscale::resolve_binary(&cfg).ok()
        };
        let Some(binary) = binary else {
            return;
        };

        while running(&app, generation) {
            let health = tailscale::ping(&binary, &ip);
            if !running(&app, generation) {
                break;
            }
            let state = {
                let session = app.state::<Session>();
                let mut inner = session.inner.lock().unwrap();
                if inner.generation != generation {
                    break;
                }
                inner.state.peer_health = Some(health);
                inner.state.clone()
            };
            emit(&app, &state);
            std::thread::sleep(HEALTH_INTERVAL);
        }
    });
}

fn running(app: &AppHandle, generation: u64) -> bool {
    let session = app.state::<Session>();
    let inner = session.inner.lock().unwrap();
    inner.generation == generation && !inner.running.is_empty()
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
        let mut outcomes = Vec::new();

        for mut entry in running {
            match entry.child.try_wait() {
                Ok(Some(status)) => {
                    changed = true;
                    finish_slot(&mut inner.state, entry.role, status.code());
                }
                Ok(None) => still.push(entry),
                Err(_) => {
                    changed = true;
                    finish_slot(&mut inner.state, entry.role, None);
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

        if let Some(dir) = inner.overlay_dir.clone() {
            let current = results::read(&dir);
            if let Some(counter) = inner.score_counter.as_mut() {
                outcomes = counter.observe(current.p1_score, current.p2_score);
            }
            if current != inner.last_result {
                inner.state.result = if current.is_empty() {
                    None
                } else {
                    Some(current.clone())
                };
                inner.last_result = current;
                changed = true;
            }
        }

        let opponent = inner.score_opponent.clone();
        let state = inner.state.clone();
        drop(inner);

        if changed {
            emit(&app, &state);
        }

        if !outcomes.is_empty() {
            if let Some(opponent) = opponent {
                let board = app.state::<ScoreBoard>();
                let now = now_ms();
                for outcome in &outcomes {
                    board.record(&opponent, *outcome, now);
                }
                app.emit(SCORES_EVENT, board.snapshot()).ok();
            }
        }

        if done {
            break;
        }
    });
}

fn finish_slot(state: &mut MatchState, role: Role, code: Option<i32>) {
    if let Some(slot) = state
        .instances
        .iter_mut()
        .find(|slot| slot.role == role && slot.pid.is_some())
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
