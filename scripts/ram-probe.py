#!/usr/bin/env python3
"""Record the sfiii3nr1 P1/P2 structs + board region for the CPU-vs-player hunt.

The goal is a byte that says "P2 is the CPU, not the remote peer" during a live
round (docs/10-lobby-spike.md L15). Record continuously while a labelled 2P match
and a labelled CPU match are played, then diff the windows with ram-window.py.

Usage: ram-probe.py <out.csv> [interval_seconds] [port]

Addresses are RAM offsets (code address - 0x02000000). Scalars/regions that are
validated on our ROM: health, game_phase, select timer, round counter. The
struct bases are the community values shifted by the health delta (-3).
"""
import socket
import sys
import time

OUT = sys.argv[1] if len(sys.argv) > 1 else "ram-probe.csv"
INTERVAL = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 55355

SCALARS = [
    ("p1_hp", 0x068D08),
    ("p2_hp", 0x0691A0),
    ("phase", 0x0154A4),
    ("p2_ctrl", 0x069104),
    ("mode", 0x015572),
    ("rounds", 0x010D28),
    ("sel_timer", 0x0154FC),
    ("credits", 0x007CE0),
]

# (name, base, length). Chunked at 256 B per READ_CORE_RAM.
REGIONS = [
    ("p1struct", 0x068C00, 0x300),
    ("p2struct", 0x069100, 0x300),
    ("board", 0x011300, 0x100),
    ("selstate", 0x015400, 0x200),
]

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(1.0)


def read(addr, length):
    sock.sendto(
        ("READ_CORE_RAM %x %d\n" % (addr, length)).encode(),
        ("127.0.0.1", PORT),
    )
    for _ in range(10):
        try:
            reply = sock.recv(65535).decode(errors="replace").split()
        except socket.timeout:
            return ""
        try:
            got = int(reply[1], 16)
        except (IndexError, ValueError):
            continue
        if got != addr:
            continue
        if len(reply) < 3 or reply[2] == "-1":
            return ""
        return "".join(reply[2:])
    return ""


def read_region(base, length):
    out = ""
    off = 0
    while off < length:
        chunk = min(0x100, length - off)
        got = read(base + off, chunk)
        if not got:
            return out
        out += got
        off += chunk
    return out


with open(OUT, "w") as f:
    f.write("epoch_ms,iso," + ",".join(name for name, _ in SCALARS))
    f.write("," + ",".join(name for name, _, _ in REGIONS) + "\n")
    f.flush()
    while True:
        row = [str(int(time.time() * 1000)), time.strftime("%H:%M:%S")]
        row += [read(addr, 1) for _, addr in SCALARS]
        row += [read_region(base, length) for _, base, length in REGIONS]
        f.write(",".join(row) + "\n")
        f.flush()
        time.sleep(INTERVAL)
