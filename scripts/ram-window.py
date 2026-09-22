#!/usr/bin/env python3
"""Diff two labelled time windows of a ram-probe.py CSV.

Usage:
  ram-window.py <csv> A=HH:MM:SS-HH:MM:SS B=HH:MM:SS-HH:MM:SS

For every byte (scalars and region offsets), report the ones that are constant
inside each window and differ between them -- the discriminator candidates.
"""
import csv
import sys

SCALARS = ["p1_hp", "p2_hp", "phase", "p2_ctrl", "mode", "rounds", "sel_timer", "credits"]
REGIONS = {
    "p1struct": 0x068C00,
    "p2struct": 0x069100,
    "board": 0x011300,
    "selstate": 0x015400,
}


def secs(stamp):
    h, m, s = stamp.split(":")
    return int(h) * 3600 + int(m) * 60 + int(s)


def parse_window(text):
    label, span = text.split("=", 1)
    start, end = span.split("-", 1)
    return label, secs(start), secs(end)


def load(path):
    rows = []
    with open(path) as f:
        reader = csv.reader(f)
        header = next(reader)
        for row in reader:
            if len(row) < len(header) or not row[1]:
                continue
            rows.append((secs(row[1]), row))
    return header, rows


def fields(header, row):
    out = {}
    for name in SCALARS:
        if name not in header:
            continue
        value = row[header.index(name)]
        if value:
            out[(name, 0)] = int(value, 16)
    for name, base in REGIONS.items():
        blob = row[header.index(name)]
        for i in range(len(blob) // 2):
            out[(name, i)] = int(blob[2 * i : 2 * i + 2], 16)
    return out


def values_in(rows, header, window):
    _, start, end = window
    sets = {}
    count = 0
    for stamp, row in rows:
        if start <= stamp <= end:
            count += 1
            for key, value in fields(header, row).items():
                sets.setdefault(key, set()).add(value)
    return sets, count


def main():
    path = sys.argv[1]
    windows = [parse_window(arg) for arg in sys.argv[2:]]
    if len(windows) < 2:
        sys.exit("need at least two windows: A=HH:MM:SS-HH:MM:SS B=...")
    header, rows = load(path)
    per_window = [values_in(rows, header, w) for w in windows]

    for (label, _, _), (sets, count) in zip(windows, per_window):
        print(f"window {label}: {count} samples")
    print()

    keys = sorted(set().union(*(sets for sets, _ in per_window)))
    limit = 40
    hits = 0
    shown = 0
    for key in keys:
        consts = []
        for sets, _ in per_window:
            vals = sets.get(key)
            if not vals or len(vals) != 1:
                consts.append(None)
            else:
                consts.append(next(iter(vals)))
        if any(c is None for c in consts):
            continue
        if len(set(consts)) < 2:
            continue
        hits += 1
        if shown >= limit:
            continue
        name, off = key
        addr = SCALARS.index(name) if name in SCALARS else REGIONS[name] + off
        where = f"{name}[0x{off:03X}]" if name in REGIONS else name
        base = f"0x{addr:06X}" if name not in SCALARS else "scalar"
        vals = "  ".join(f"{label}=0x{v:02X}" for (label, _, _), v in zip(windows, consts))
        print(f"  {where:<22} {base:<10} {vals}")
        shown += 1
    if hits > shown:
        print(f"  ... ({hits - shown} more; raise the limit in the script to see them)")
    print(f"\n{hits} byte(s) constant per window and different across them")


if __name__ == "__main__":
    main()
