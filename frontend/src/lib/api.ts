import { invoke } from "@tauri-apps/api/core";
import type {
  CabinetMode,
  CabinetRect,
  CabinetStatus,
  Config,
  DiagnosticsResult,
  DiscoveredRoom,
  DownloadedCore,
  JoinOutcome,
  LaunchRequest,
  LobbyJoinRequest,
  LobbyStartRequest,
  MatchState,
  ParityStatus,
  PeerHealth,
  PortProbe,
  ProviderInfo,
  RetroArchInputMap,
  Room,
  RomIndex,
  Tailnet,
} from "./types";


export const getConfig = () => invoke<Config>("get_config");

export const setConfig = (config: Config) =>
  invoke<Config>("set_config", { config });

export const listPeers = () => invoke<Tailnet>("list_peers");

export const peersHealth = (ips: string[]) =>
  invoke<PeerHealth[]>("peers_health", { ips });

export const listRoms = () => invoke<RomIndex>("list_roms");

export const launcherInfo = () => invoke<ProviderInfo>("launcher_info");

export const parityStatus = (rom: string) =>
  invoke<ParityStatus | null>("parity_status", { rom });

export const launchMatch = (request: LaunchRequest) =>
  invoke<MatchState>("launch_match", { request });

export const launchDevPair = (rom: string) =>
  invoke<MatchState>("launch_dev_pair", { rom });

export const downloadRetroArchCore = () =>
  invoke<DownloadedCore>("download_retroarch_core");

export const getRetroArchHotkeys = () =>
  invoke<RetroArchInputMap>("retroarch_hotkey_map");

export const stopMatch = () => invoke<MatchState>("stop_match");

export const matchStatus = () => invoke<MatchState>("match_status");

export const cabinetStatus = () => invoke<CabinetStatus>("cabinet_status");

export const cabinetPlace = (windowId: number, rect: CabinetRect) =>
  invoke<CabinetMode>("cabinet_place", { windowId, rect });

export const cabinetRelease = (windowId: number) =>
  invoke<void>("cabinet_release", { windowId });

export const cabinetRequestPermission = () =>
  invoke<boolean>("cabinet_request_permission");

export const lobbyStart = (request: LobbyStartRequest) =>
  invoke<Room>("lobby_start", { request });

export const lobbyStop = () => invoke<void>("lobby_stop");

export const lobbyStatus = () => invoke<Room | null>("lobby_status");

export const lobbyRoom = () => invoke<Room | null>("lobby_room");

export const lobbyQuery = (host: string) =>
  invoke<Room | null>("lobby_query", { host });

export const lobbyDiscover = () => invoke<DiscoveredRoom[]>("lobby_discover");

export const lobbyJoin = (request: LobbyJoinRequest) =>
  invoke<JoinOutcome>("lobby_join", { request });

export const probePort = (ip: string, port: number, timeoutMs?: number) =>
  invoke<PortProbe>("probe_port", { ip, port, timeoutMs });

export const openLogsDir = () => invoke<string>("open_logs_dir");

export const collectDiagnostics = () =>
  invoke<DiagnosticsResult>("collect_diagnostics");

export const logFrontend = (
  level: "error" | "warn" | "info" | "debug",
  message: string,
  context?: string,
) => invoke<void>("log_frontend", { level, message, context });
