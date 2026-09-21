use crate::config::{self, Config};
use crate::constants;
use crate::contracts::LaunchSpec;
use crate::logging;
use crate::probe;
use crate::providers::Role;
use crate::sync::MutexExt;
use crate::tailscale::{self, PeerHealth};
use crate::time;
use serde::Serialize;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub const MATCH_EVENT: &str = "match-state-changed";

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
}

#[derive(Default)]
pub struct Session {
    inner: Mutex<SessionInner>,
}

fn emit(app: &AppHandle, state: &MatchState) {
    app.emit(MATCH_EVENT, state).ok();
}

fn spawn_child(plan: &Plan, session_dir: Option<&Path>) -> crate::error::Result<Child> {
    let mut command = crate::process::command(&plan.spec.program);
    command
        .args(&plan.spec.args)
        .current_dir(&plan.spec.cwd)
        .envs(plan.spec.envs.iter().cloned());
    if session_dir.is_some() {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to launch {}: {err}", plan.spec.program.display()))?;
    if let Some(dir) = session_dir {
        match logging::open_emulator_log(dir, plan.role.key()) {
            Ok(sink) => {
                let label = format!("{}[{}]", plan.role.key(), plan.rom);
                if let Some(stdout) = child.stdout.take() {
                    logging::capture_stream(stdout, format!("{label} stdout"), sink.clone());
                }
                if let Some(stderr) = child.stderr.take() {
                    logging::capture_stream(stderr, format!("{label} stderr"), sink);
                }
            }
            Err(err) => log::warn!("cannot capture emulator output: {err}"),
        }
    }
    Ok(child)
}

fn clear_running(inner: &mut SessionInner) {
    for mut running in inner.running.drain(..) {
        let _ = running.child.kill();
        let _ = running.child.wait();
    }
}

#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub dev: bool,
    pub wait_for_host: bool,
    pub peer_display: String,
}

pub fn launch_many(
    app: &AppHandle,
    plans: &[Plan],
    options: LaunchOptions,
) -> crate::error::Result<MatchState> {
    if plans.is_empty() {
        return Err("nothing to launch".into());
    }

    let session = app.state::<Session>();
    let mut inner = session.inner.lock_or_recover();
    clear_running(&mut inner);
    inner.generation += 1;
    let generation = inner.generation;

    let session_dir = match logging::create_session_dir(app) {
        Ok(dir) => {
            log::info!("session log dir: {}", dir.display());
            match logging::prune_sessions(app) {
                Ok(removed) if removed > 0 => {
                    log::info!("pruned {removed} old session dir(s)")
                }
                Ok(_) => {}
                Err(err) => log::warn!("cannot prune old session dirs: {err}"),
            }
            Some(dir)
        }
        Err(err) => {
            log::warn!("{err}");
            None
        }
    };

    let mut running: Vec<Running> = Vec::new();
    let mut instances = Vec::new();

    for plan in plans {
        if options.wait_for_host && matches!(plan.role, Role::P2 | Role::Spectator) {
            if let Some(port) = plan.port {
                log::info!(
                    "waiting for host {}:{} before starting {}",
                    plan.peer_ip,
                    port,
                    plan.role.label()
                );
                let ready = probe::wait_for_port(&plan.peer_ip, port, constants::HOST_WAIT_TIMEOUT);
                log::info!("host {}:{} reachable={ready}", plan.peer_ip, port);
                if !ready {
                    log::warn!(
                        "host {}:{} did not accept connections; {} may show \"Failed to initialize netplay\"",
                        plan.peer_ip,
                        port,
                        plan.role.label()
                    );
                }
            }
        }
        log::info!(
            "launching {} program={} cwd={} args={:?}",
            plan.role.label(),
            plan.spec.program.display(),
            plan.spec.cwd.display(),
            plan.spec.args
        );
        let child = match spawn_child(plan, session_dir.as_deref()) {
            Ok(child) => child,
            Err(err) => {
                log::error!("{err}");
                for mut started in running.drain(..) {
                    let _ = started.child.kill();
                    let _ = started.child.wait();
                }
                inner.state = MatchState::default();
                return Err(err);
            }
        };
        let pid = child.id();
        log::info!("spawned {} pid={}", plan.role.label(), pid);
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
        peer_ip: Some(options.peer_display),
        dev: options.dev,
        started_at_ms: Some(time::now_ms()),
        instances,
        peer_health: None,
        message: None,
    };

    inner.running = running;
    inner.state = state.clone();
    drop(inner);

    emit(app, &state);
    spawn_monitor(app.clone(), generation);
    if !options.dev && !plans[0].peer_ip.trim().is_empty() {
        spawn_health(app.clone(), generation, plans[0].peer_ip.clone());
    }
    Ok(state)
}

pub fn launch(
    app: &AppHandle,
    plan: &Plan,
    options: LaunchOptions,
) -> crate::error::Result<MatchState> {
    launch_many(app, std::slice::from_ref(plan), options)
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
                let mut inner = session.inner.lock_or_recover();
                if inner.generation != generation {
                    break;
                }
                inner.state.peer_health = Some(health);
                inner.state.clone()
            };
            emit(&app, &state);
            std::thread::sleep(constants::HEALTH_INTERVAL);
        }
    });
}

fn running(app: &AppHandle, generation: u64) -> bool {
    let session = app.state::<Session>();
    let inner = session.inner.lock_or_recover();
    inner.generation == generation && !inner.running.is_empty()
}

fn spawn_monitor(app: AppHandle, generation: u64) {
    std::thread::spawn(move || loop {
        std::thread::sleep(constants::MONITOR_INTERVAL);

        let session = app.state::<Session>();
        let mut inner = session.inner.lock_or_recover();
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
                    log::info!(
                        "{} exited with code {:?}",
                        entry.role.label(),
                        status.code()
                    );
                    finish_slot(&mut inner.state, entry.role, status.code());
                }
                Ok(None) => still.push(entry),
                Err(err) => {
                    changed = true;
                    log::warn!("{} status error: {err}", entry.role.label());
                    finish_slot(&mut inner.state, entry.role, None);
                }
            }
        }

        inner.running = still;
        let done = inner.running.is_empty();
        if done && inner.state.status == "running" {
            inner.state.status = "finished".to_string();
            inner.state.message = Some("all instances exited".to_string());
            log::info!("match finished: all instances exited");
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

pub fn stop(app: &AppHandle) -> crate::error::Result<MatchState> {
    let session = app.state::<Session>();
    let mut inner = session.inner.lock_or_recover();
    inner.generation += 1;
    let had_running = !inner.running.is_empty();
    clear_running(&mut inner);

    if had_running {
        log::info!("stopping match");
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
    app.state::<Session>().inner.lock_or_recover().state.clone()
}
