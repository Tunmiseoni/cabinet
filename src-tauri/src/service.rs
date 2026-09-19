use crate::commands;
use crate::config::Config;
use crate::control::{ClientMessage, Confidence, ControlClient, ControlServer, Outcome, Want};
use crate::discovery::{Advertiser, RoomAdvert};
use crate::player::{self, Player};
use crate::room::{CurrentMatch, Phase, RoomState};
use crate::session;
use crate::tailscale;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const ROOM_EVENT: &str = "room-state-changed";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Hosting,
    Joined,
}

struct ActiveRoom {
    role: Role,
    self_player: Player,
    rom: String,
    client: Arc<Mutex<ControlClient>>,
    advertiser: Option<Arc<Advertiser>>,
    announced: Arc<Mutex<Option<String>>>,
    secret: Option<String>,
    generation: u64,
}

#[derive(Default)]
struct RoomInner {
    active: Option<ActiveRoom>,
    generation: u64,
}

#[derive(Default)]
pub struct RoomService {
    inner: Mutex<RoomInner>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn random_secret() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut seed = now_ms()
        ^ ((std::process::id() as u64) << 32)
        ^ (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(12345));
    if seed == 0 {
        seed = 0x9E3779B97F4A7C15;
    }
    (0..6)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ALPHABET[(seed % ALPHABET.len() as u64) as usize] as char
        })
        .collect()
}

fn advert_for(state: &RoomState, secret_required: bool) -> RoomAdvert {
    RoomAdvert {
        room_id: state.room_id.clone(),
        host: state.host.handle.clone(),
        rom: state.rom.clone(),
        phase: match state.phase {
            Phase::Lobby => "lobby".to_string(),
            Phase::Playing => "playing".to_string(),
        },
        champion: state.champion.as_ref().map(|p| p.handle.clone()),
        queue: state.queue.len() as u32,
        players: state.ledger.len() as u32,
        secret_required,
    }
}

fn resolve_self_player(app: &AppHandle) -> Result<Player, String> {
    let cfg = Config::load(&commands::config_file(app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    let tailnet = tailscale::status(&binary)?;
    player::self_player(&cfg, &tailnet).ok_or_else(|| "could not determine this machine's tailnet identity".to_string())
}

fn launch_room_match(
    app: &AppHandle,
    rom: &str,
    peer_ip: &str,
    side: u8,
) -> Result<(), String> {
    let cfg = Config::load(&commands::config_file(app)?);
    let launcher = commands::resolve_launcher(&cfg, false)?;
    let config = crate::launcher::MatchConfig::new(rom.to_string(), peer_ip.to_string(), side)?;
    let spec = launcher.spec(&config)?;
    session::launch(app, &spec, &config, false)?;
    Ok(())
}

fn reconcile(app: &AppHandle, state: &RoomState, rom: &str, self_player: &Player, announced: &Mutex<Option<String>>) {
    let self_id = self_player.node_id.as_str();
    let desired = state
        .current_match
        .as_ref()
        .filter(|current| current.names(self_id))
        .map(|current: &CurrentMatch| {
            if current.p1.node_id == self_id {
                (current.match_id.clone(), current.p1.side, current.p2.ip.clone())
            } else {
                (current.match_id.clone(), current.p2.side, current.p1.ip.clone())
            }
        });

    let mut announced = announced.lock().unwrap();
    match desired {
        Some((match_id, side, peer_ip)) => {
            if announced.as_deref() != Some(match_id.as_str()) {
                if let Err(err) = launch_room_match(app, rom, &peer_ip, side) {
                    eprintln!("room: failed to launch match: {err}");
                }
                *announced = Some(match_id);
            }
        }
        None => {
            if announced.is_some() {
                let _ = session::stop(app);
                *announced = None;
            }
        }
    }
}

impl RoomService {
    fn emit_state(app: &AppHandle, state: Option<&RoomState>) {
        app.emit(ROOM_EVENT, state).ok();
    }

    pub fn state(&self) -> Option<RoomState> {
        let inner = self.inner.lock().unwrap();
        inner
            .active
            .as_ref()
            .and_then(|active| active.client.lock().unwrap().state().clone().into())
    }

    pub fn secret(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner
            .active
            .as_ref()
            .filter(|active| active.role == Role::Hosting)
            .and_then(|active| active.secret.clone())
    }

    pub fn host(app: &AppHandle, rom: String, secret: Option<String>) -> Result<RoomState, String> {
        if rom.trim().is_empty() {
            return Err("pick a ROM to host".into());
        }
        let self_player = resolve_self_player(app)?;
        let cfg = Config::load(&commands::config_file(app)?);
        let secret = secret
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(random_secret);
        let room_id = self_player.node_id.clone();

        Self::teardown(app);

        let room = RoomState::new(room_id.clone(), self_player.clone(), rom.trim());
        let server = ControlServer::bind(room, Some(secret.clone()), cfg.control_port)
            .map_err(|err| format!("cannot bind control port {}: {err}", cfg.control_port))?;
        let server = server.spawn();

        let advertiser = Advertiser::bind(cfg.discovery_port)
            .map_err(|err| format!("cannot bind discovery port {}: {err}", cfg.discovery_port))?;
        let advertiser = advertiser.spawn();
        advertiser.set(Some(advert_for(&server.state(), true)));

        let client = ControlClient::connect(
            "127.0.0.1",
            cfg.control_port,
            &room_id,
            &self_player,
            Some(&secret),
            Want::Play,
            CONNECT_TIMEOUT,
        )
        .map_err(|err| format!("cannot connect to own control channel: {err}"))?;

        let generation = {
            let service = app.state::<RoomService>();
            let mut inner = service.inner.lock().unwrap();
            inner.generation += 1;
            inner.generation
        };

        let state = client.state().clone();
        {
            let service = app.state::<RoomService>();
            let mut inner = service.inner.lock().unwrap();
            inner.active = Some(ActiveRoom {
                role: Role::Hosting,
                self_player: self_player.clone(),
                rom: rom.trim().to_string(),
                client: Arc::new(Mutex::new(client)),
                advertiser: Some(advertiser),
                announced: Arc::new(Mutex::new(None)),
                secret: Some(secret.clone()),
                generation,
            });
        }

        Self::spawn_poller(app.clone(), generation);
        Self::emit_state(app, Some(&state));
        Ok(state)
    }

    pub fn join(
        app: &AppHandle,
        ip: String,
        room_id: String,
        secret: Option<String>,
    ) -> Result<RoomState, String> {
        let self_player = resolve_self_player(app)?;
        let cfg = Config::load(&commands::config_file(app)?);
        let secret = secret
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Self::teardown(app);

        let client = ControlClient::connect(
            &ip,
            cfg.control_port,
            &room_id,
            &self_player,
            secret.as_deref(),
            Want::Play,
            CONNECT_TIMEOUT,
        )
        .map_err(|err| format!("cannot join room: {err}"))?;

        let generation = {
            let service = app.state::<RoomService>();
            let mut inner = service.inner.lock().unwrap();
            inner.generation += 1;
            inner.generation
        };

        let state = client.state().clone();
        {
            let service = app.state::<RoomService>();
            let mut inner = service.inner.lock().unwrap();
            inner.active = Some(ActiveRoom {
                role: Role::Joined,
                self_player,
                rom: state.rom.clone(),
                client: Arc::new(Mutex::new(client)),
                advertiser: None,
                announced: Arc::new(Mutex::new(None)),
                secret: None,
                generation,
            });
        }

        Self::spawn_poller(app.clone(), generation);
        Self::emit_state(app, Some(&state));
        Ok(state)
    }

    pub fn leave(app: &AppHandle) -> Result<(), String> {
        Self::teardown(app);
        Self::emit_state(app, None);
        Ok(())
    }

    fn teardown(app: &AppHandle) {
        let service = app.state::<RoomService>();
        let active = {
            let mut inner = service.inner.lock().unwrap();
            inner.generation += 1;
            inner.active.take()
        };
        if let Some(active) = active {
            if let Some(advertiser) = &active.advertiser {
                advertiser.set(None);
            }
            let _ = session::stop(app);
        }
    }

    fn with_client<F>(app: &AppHandle, action: F) -> Result<(), String>
    where
        F: FnOnce(&mut ControlClient) -> Result<(), String>,
    {
        let service = app.state::<RoomService>();
        let client = {
            let inner = service.inner.lock().unwrap();
            inner
                .active
                .as_ref()
                .map(|active| Arc::clone(&active.client))
        };
        let client = client.ok_or_else(|| "not in a room".to_string())?;
        let mut guard = client.lock().unwrap();
        action(&mut guard)
    }

    pub fn enqueue(app: &AppHandle) -> Result<(), String> {
        Self::with_client(app, |client| {
            client
                .send(&ClientMessage::Enqueue)
                .map_err(|err| err.to_string())
        })
    }

    pub fn leave_queue(app: &AppHandle) -> Result<(), String> {
        Self::with_client(app, |client| {
            client
                .send(&ClientMessage::Leave)
                .map_err(|err| err.to_string())
        })
    }

    pub fn report_result(app: &AppHandle, match_id: String, won: bool) -> Result<(), String> {
        Self::with_client(app, |client| {
            client
                .send(&ClientMessage::Result {
                    match_id,
                    outcome: if won { Outcome::Win } else { Outcome::Loss },
                    confidence: Confidence::Manual,
                })
                .map_err(|err| err.to_string())
        })
    }

    fn spawn_poller(app: AppHandle, generation: u64) {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(200));

            let (client, rom, self_player, advertiser, announced, role) = {
                let service = app.state::<RoomService>();
                let inner = service.inner.lock().unwrap();
                match &inner.active {
                    Some(active) if active.generation == generation => (
                        Arc::clone(&active.client),
                        active.rom.clone(),
                        active.self_player.clone(),
                        active.advertiser.clone(),
                        Arc::clone(&active.announced),
                        active.role,
                    ),
                    _ => break,
                }
            };

            let polled = client
                .lock()
                .unwrap()
                .poll(Duration::from_millis(500));

            match polled {
                Ok(Some(state)) => {
                    if role == Role::Hosting {
                        if let Some(advertiser) = &advertiser {
                            advertiser.set(Some(advert_for(&state, true)));
                        }
                    }
                    reconcile(&app, &state, &rom, &self_player, &announced);
                    Self::emit_state(&app, Some(&state));
                }
                Ok(None) => {}
                Err(err) => eprintln!("room: control channel error: {err}"),
            }
        });
    }
}
