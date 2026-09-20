export type ProviderKind = "fightcade" | "retroarch";

export type MatchRole = "p1" | "p2" | "spectator";

export interface Config {
  handle: string | null;
  fightcadeDir: string | null;
  romDir: string | null;
  tailscalePath: string | null;
  defaultPeerIp: string | null;
  discoveryPort: number;
  controlPort: number;
  rttWarnMs: number;
  pollIntervalSecs: number;
  cabinetMode: boolean;
  provider: ProviderKind;
  retroarchPath: string | null;
  retroarchCore: string | null;
  retroarchPort: number;
  retroarchNickname: string | null;
  verboseLogging: boolean;
  developerMode: boolean;
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
  overlayResults: boolean;
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

export interface RoomAdvert {
  roomId: string;
  host: string;
  rom: string;
  phase: string;
  champion: string | null;
  queue: number;
  players: number;
  secretRequired: boolean;
}

export interface DiscoveredRoom extends RoomAdvert {
  ip: string;
}

export interface RoomPlayer {
  nodeId: string;
  handle: string;
  ip: string;
}

export interface MatchSlot {
  nodeId: string;
  handle: string;
  ip: string;
  side: number;
}

export interface CurrentMatch {
  matchId: string;
  p1: MatchSlot;
  p2: MatchSlot;
  startedAtMs: number;
}

export interface LedgerEntry {
  handle: string;
  wins: number;
  losses: number;
  draws: number;
  games: number;
}

export type RoomPhase = "lobby" | "playing";

export interface RoomState {
  roomId: string;
  host: RoomPlayer;
  rom: string;
  revision: number;
  phase: RoomPhase;
  champion: RoomPlayer | null;
  challenger: RoomPlayer | null;
  queue: RoomPlayer[];
  currentMatch: CurrentMatch | null;
  ledger: Record<string, LedgerEntry>;
  matchSeq: number;
}

export interface OverlayStatus {
  enabled: boolean;
  iniPath: string;
}

export interface ScoreEntry {
  wins: number;
  losses: number;
  draws: number;
  games: number;
  currentStreak: number;
  bestStreak: number;
  lastPlayedMs: number;
}

export interface ScoreRecord extends ScoreEntry {
  opponent: string;
}

export interface ScoreSnapshot {
  records: ScoreRecord[];
  totals: ScoreEntry;
}

export interface InstanceState {
  role: MatchRole;
  roleLabel: string;
  port: number | null;
  pid: number | null;
  exitCode: number | null;
  message: string | null;
}

export interface MatchResult {
  rom: string | null;
  started: boolean;
  winner: string | null;
  winnerSide: number | null;
  p1Name: string | null;
  p2Name: string | null;
  p1Score: number | null;
  p2Score: number | null;
  p1Character: string | null;
  p2Character: string | null;
}

export interface MatchState {
  status: "idle" | "running" | "finished";
  rom: string | null;
  peerIp: string | null;
  dev: boolean;
  startedAtMs: number | null;
  instances: InstanceState[];
  result: MatchResult | null;
  peerHealth: PeerHealth | null;
  message: string | null;
}

export interface LaunchRequest {
  rom: string;
  peerIp: string;
  role: MatchRole;
  dev: boolean;
  force?: boolean;
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

export const SCORES_EVENT = "scores-changed";

export const ROOM_EVENT = "room-state-changed";
