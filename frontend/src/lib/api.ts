import { invoke } from "@tauri-apps/api/core";

export interface Config {
  fightcadeDir: string | null;
  romDir: string | null;
  tailscalePath: string | null;
  defaultPeerIp: string | null;
  rttWarnMs: number;
  pollIntervalSecs: number;
}

export interface Peer {
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

export interface InstanceState {
  side: number;
  sideLabel: string;
  port: number;
  pid: number | null;
  exitCode: number | null;
  message: string | null;
}

export interface MatchState {
  status: "idle" | "running" | "finished";
  rom: string | null;
  peerIp: string | null;
  dev: boolean;
  startedAtMs: number | null;
  instances: InstanceState[];
  message: string | null;
}

export interface LaunchRequest {
  rom: string;
  peerIp: string;
  side: number;
  dev: boolean;
}

export const MATCH_EVENT = "match-state-changed";

export const getConfig = () => invoke<Config>("get_config");

export const setConfig = (config: Config) =>
  invoke<Config>("set_config", { config });

export const listPeers = () => invoke<Tailnet>("list_peers");

export const peerHealth = (ip: string) =>
  invoke<PeerHealth>("peer_health", { ip });

export const peersHealth = (ips: string[]) =>
  invoke<PeerHealth[]>("peers_health", { ips });

export const listRoms = () => invoke<RomIndex>("list_roms");

export const launcherInfo = () => invoke<InstallInfo>("launcher_info");

export const launchMatch = (request: LaunchRequest) =>
  invoke<MatchState>("launch_match", { request });

export const launchDevPair = (rom: string) =>
  invoke<MatchState>("launch_dev_pair", { rom });

export const stopMatch = () => invoke<MatchState>("stop_match");

export const matchStatus = () => invoke<MatchState>("match_status");
