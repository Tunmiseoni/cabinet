import { invoke } from "@tauri-apps/api/core";
import type {
  CabinetMode,
  CabinetRect,
  CabinetStatus,
  Config,
  DiagnosticsResult,
  DiscoveredRoom,
  LaunchRequest,
  MatchState,
  OverlayStatus,
  ParityStatus,
  PeerHealth,
  PortProbe,
  ProviderInfo,
  RomIndex,
  RoomState,
  ScoreSnapshot,
  Tailnet,
} from "./types";

export * from "./types";

export const getConfig = () => invoke<Config>("get_config");

export const setConfig = (config: Config) =>
  invoke<Config>("set_config", { config });

export const listPeers = () => invoke<Tailnet>("list_peers");

export const peerHealth = (ip: string) =>
  invoke<PeerHealth>("peer_health", { ip });

export const peersHealth = (ips: string[]) =>
  invoke<PeerHealth[]>("peers_health", { ips });

export const listRoms = () => invoke<RomIndex>("list_roms");

export const listRooms = () => invoke<DiscoveredRoom[]>("list_rooms");

export const launcherInfo = () => invoke<ProviderInfo>("launcher_info");

export const overlayStatus = () => invoke<OverlayStatus>("overlay_status");

export const parityStatus = (rom: string) =>
  invoke<ParityStatus | null>("parity_status", { rom });

export const enableOverlay = () => invoke<OverlayStatus>("enable_overlay");

export const getScores = () => invoke<ScoreSnapshot>("get_scores");

export const resetScores = () => invoke<ScoreSnapshot>("reset_scores");

export const launchMatch = (request: LaunchRequest) =>
  invoke<MatchState>("launch_match", { request });

export const launchDevPair = (rom: string) =>
  invoke<MatchState>("launch_dev_pair", { rom });

export const stopMatch = () => invoke<MatchState>("stop_match");

export const matchStatus = () => invoke<MatchState>("match_status");

export const hostRoom = (rom: string, secret: string | null) =>
  invoke<RoomState>("host_room", { rom, secret });

export const joinRoom = (
  ip: string,
  roomId: string,
  secret: string | null,
) => invoke<RoomState>("join_room", { ip, roomId, secret });

export const leaveRoom = () => invoke<void>("leave_room");

export const roomEnqueue = () => invoke<void>("room_enqueue");

export const roomLeaveQueue = () => invoke<void>("room_leave_queue");

export const reportRoomResult = (matchId: string, won: boolean) =>
  invoke<void>("report_room_result", { matchId, won });

export const roomState = () => invoke<RoomState | null>("room_state");

export const roomSecret = () => invoke<string | null>("room_secret");

export const cabinetStatus = () => invoke<CabinetStatus>("cabinet_status");

export const cabinetPlace = (windowId: number, rect: CabinetRect) =>
  invoke<CabinetMode>("cabinet_place", { windowId, rect });

export const cabinetRelease = (windowId: number) =>
  invoke<void>("cabinet_release", { windowId });

export const cabinetRequestPermission = () =>
  invoke<boolean>("cabinet_request_permission");

export const probePort = (ip: string, port: number, timeoutMs?: number) =>
  invoke<PortProbe>("probe_port", { ip, port, timeoutMs });

export const logDir = () => invoke<string>("log_dir");

export const openLogsDir = () => invoke<string>("open_logs_dir");

export const collectDiagnostics = () =>
  invoke<DiagnosticsResult>("collect_diagnostics");

export const logFrontend = (
  level: "error" | "warn" | "info" | "debug",
  message: string,
  context?: string,
) => invoke<void>("log_frontend", { level, message, context });
