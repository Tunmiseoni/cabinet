export type ProviderKind = "fightcade" | "retroarch";

export type MatchRole = "p1" | "p2" | "spectator";

export interface Config {
  handle: string | null;
  fightcadeDir: string | null;
  romDir: string | null;
  tailscalePath: string | null;
  defaultPeerIp: string | null;
  rttWarnMs: number;
  pollIntervalSecs: number;
  cabinetMode: boolean;
  provider: ProviderKind;
  retroarchPath: string | null;
  retroarchCore: string | null;
  retroarchPort: number;
  retroarchCommandPort: number;
  retroarchNickname: string | null;
  retroarchMuteSpectators: boolean;
  retroarchMaxPingMs: number;
  retroarchIsolatedConfig: boolean;
  retroarchInput: RetroArchInput;
  retroarchInputEnabled: boolean;
  lobbyBeaconPort: number;
  verboseLogging: boolean;
  developerMode: boolean;
}

export interface RetroArchInput {
  up: string;
  down: string;
  left: string;
  right: string;
  lightPunch: string;
  mediumPunch: string;
  heavyPunch: string;
  lightKick: string;
  mediumKick: string;
  heavyKick: string;
  start: string;
  coin: string;
}

export interface HotkeyBinding {
  action: string;
  configKey: string;
  key: string;
  collides: boolean;
}

export interface RetroArchInputMap {
  bindings: HotkeyBinding[];
  defaults: RetroArchInput;
}

export interface Peer {
  nodeId: string;
  hostname: string;
  dnsName: string;
  os: string;
  ip: string;
  ips: string[];
  online: boolean;
  active: boolean;
  curAddr: string;
  relay: string;
  rxBytes: number;
  txBytes: number;
  lastSeen: string;
  isSelf: boolean;
}

export interface Tailnet {
  backendState: string;
  selfPeer: Peer | null;
  peers: Peer[];
}

export type PathKind = "direct" | "relay" | "local" | "unknown";

export interface PeerHealth {
  ip: string;
  ok: boolean;
  path: PathKind;
  rttMs: number | null;
  relayCode: string | null;
  raw: string;
}

export interface Rom {
  shortName: string;
  fileName: string;
  path: string;
  sizeBytes: number;
}

export interface RomIndex {
  dir: string | null;
  roms: Rom[];
}

export interface InstallInfo {
  id: string;
  label: string;
  installed: boolean;
  detail: string;
}

export interface ProviderCapabilities {
  spectate: boolean;
  devPair: boolean;
}

export interface ProviderInfo {
  kind: ProviderKind;
  install: InstallInfo;
  capabilities: ProviderCapabilities;
}

export interface ParityStatus {
  applicable: boolean;
  ok: boolean;
  corePath: string;
  coreGit: string | null;
  coreSha256: string | null;
  expectedGit: string;
  expectedSha256: string;
  romPath: string;
  romSha256: string | null;
  expectedRomSha256: string;
  detail: string;
}

export interface DownloadedCore {
  path: string;
  sha256: string;
}

export type NetplayConnection =
  | "connecting"
  | "connected"
  | "disconnected"
  | "failed";

export interface NetplayPlayer {
  nick: string;
  player: number;
  pingMs: number | null;
}

export interface NetplayEvent {
  atMs: number;
  kind: string;
  text: string;
}

export interface NetplayInfo {
  connection: NetplayConnection;
  selfPlayer: number | null;
  host: string | null;
  players: NetplayPlayer[];
  pingMs: number | null;
  coreWarning: boolean;
  lastEvent: string | null;
  events: NetplayEvent[];
}

export interface InstanceState {
  role: MatchRole;
  roleLabel: string;
  port: number | null;
  commandPort: number | null;
  pid: number | null;
  exitCode: number | null;
  message: string | null;
  netplay: NetplayInfo | null;
}

export interface MatchState {
  status: "idle" | "running" | "finished";
  rom: string | null;
  peerIp: string | null;
  dev: boolean;
  startedAtMs: number | null;
  instances: InstanceState[];
  peerHealth: PeerHealth | null;
  message: string | null;
}

export interface LaunchRequest {
  rom: string;
  peerIp: string;
  role: MatchRole;
  playerSlot?: number | null;
  dev: boolean;
  force?: boolean;
}

export type RoomPhase = "waiting" | "playing";

export interface Room {
  roomId: string;
  hostNodeId: string;
  hostHandle: string;
  rom: string;
  firstTo: number;
  phase: RoomPhase;
  players: number;
  spectators: number;
  hostSeat: number | null;
  revision: number;
}

export interface DiscoveredRoom {
  hostIp: string;
  hostHostname: string;
  room: Room;
}

export interface LobbyStartRequest {
  rom: string;
  firstTo?: number;
  hostSeat?: number | null;
}

export interface LobbyJoinRequest {
  host: string;
  rom?: string | null;
  spectate?: boolean;
  playerSlot?: number | null;
  force?: boolean;
}

export interface JoinOutcome {
  room: Room | null;
  role: MatchRole;
  playerSlot: number | null;
  state: MatchState;
}

export interface PortProbe {
  ip: string;
  port: number;
  reachable: boolean;
  latencyMs: number | null;
  error: string | null;
}

export interface DiagnosticsResult {
  path: string;
  logDir: string;
}

export type CabinetPermission = "granted" | "denied" | "notRequired";

export type CabinetMode = "placement" | "frameFollow" | "unsupported";

export interface CabinetRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CabinetWindow {
  id: number;
  ownerPid: number;
  ownerName: string;
  title: string;
  bounds: CabinetRect;
}

export interface CabinetStatus {
  platform: string;
  supported: boolean;
  permission: CabinetPermission;
  mode: CabinetMode;
  detail: string;
  ownerPids: number[];
  windows: CabinetWindow[];
}

export const MATCH_EVENT = "match-state-changed";
export const CORE_DOWNLOAD_EVENT = "core-download-progress";

export interface CoreDownloadProgress {
  downloaded: number;
  total: number | null;
}
