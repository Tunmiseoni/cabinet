use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::sync::MutexExt;

pub const SCORES_EVENT: &str = "scores-changed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Win,
    Loss,
    #[allow(dead_code)]
    Draw,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreEntry {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games: u32,
    pub current_streak: u32,
    pub best_streak: u32,
    pub last_played_ms: u64,
}

impl ScoreEntry {
    pub fn record(&mut self, outcome: Outcome, now_ms: u64) {
        match outcome {
            Outcome::Win => {
                self.wins += 1;
                self.current_streak += 1;
                self.best_streak = self.best_streak.max(self.current_streak);
            }
            Outcome::Loss => {
                self.losses += 1;
                self.current_streak = 0;
            }
            Outcome::Draw => {
                self.draws += 1;
            }
        }
        self.games += 1;
        self.last_played_ms = now_ms;
    }

    fn merge(&mut self, other: &ScoreEntry) {
        self.wins += other.wins;
        self.losses += other.losses;
        self.draws += other.draws;
        self.games += other.games;
        self.last_played_ms = self.last_played_ms.max(other.last_played_ms);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ledger {
    pub version: u32,
    pub entries: BTreeMap<String, ScoreEntry>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            version: 1,
            entries: BTreeMap::new(),
        }
    }
}

impl Ledger {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, raw)
    }

    fn snapshot(&self) -> Snapshot {
        let mut records: Vec<ScoreRecord> = self
            .entries
            .iter()
            .map(|(opponent, entry)| ScoreRecord {
                opponent: opponent.clone(),
                entry: entry.clone(),
            })
            .collect();
        records.sort_by(|a, b| {
            b.entry
                .games
                .cmp(&a.entry.games)
                .then_with(|| a.opponent.cmp(&b.opponent))
        });

        let mut totals = ScoreEntry::default();
        for entry in self.entries.values() {
            totals.merge(entry);
        }

        Snapshot { records, totals }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreRecord {
    pub opponent: String,
    #[serde(flatten)]
    pub entry: ScoreEntry,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub records: Vec<ScoreRecord>,
    pub totals: ScoreEntry,
}

pub struct ScoreBoard {
    inner: Mutex<Ledger>,
    path: PathBuf,
}

impl ScoreBoard {
    pub fn load(path: PathBuf) -> Self {
        let ledger = Ledger::load(&path);
        Self {
            inner: Mutex::new(ledger),
            path,
        }
    }

    pub fn record(&self, opponent: &str, outcome: Outcome, now_ms: u64) {
        if opponent.is_empty() {
            return;
        }
        let mut ledger = self.inner.lock_or_recover();
        ledger
            .entries
            .entry(opponent.to_string())
            .or_default()
            .record(outcome, now_ms);
        let _ = ledger.save(&self.path);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock_or_recover().snapshot()
    }

    pub fn reset(&self) -> Snapshot {
        let mut ledger = self.inner.lock_or_recover();
        *ledger = Ledger::default();
        let _ = ledger.save(&self.path);
        ledger.snapshot()
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScoreCounter {
    local_side: u8,
    last: Option<(i64, i64)>,
}

impl ScoreCounter {
    pub fn new(local_side: u8) -> Self {
        Self {
            local_side,
            last: None,
        }
    }

    pub fn observe(&mut self, p1: Option<i64>, p2: Option<i64>) -> Vec<Outcome> {
        let (p1, p2) = match (p1, p2) {
            (Some(p1), Some(p2)) => (p1, p2),
            _ => return Vec::new(),
        };

        let mut outcomes = Vec::new();
        if let Some((last_p1, last_p2)) = self.last {
            if p1 >= last_p1 && p2 >= last_p2 {
                let d1 = p1 - last_p1;
                let d2 = p2 - last_p2;
                if d1 + d2 == 1 {
                    let winner_side = if d1 == 1 { 0 } else { 1 };
                    outcomes.push(if winner_side == self.local_side {
                        Outcome::Win
                    } else {
                        Outcome::Loss
                    });
                }
            }
        }

        self.last = Some((p1, p2));
        outcomes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cabinet-scores-{}-{tag}.json", std::process::id()))
    }

    #[test]
    fn records_wins_losses_and_streaks() {
        let path = temp_path("record");
        let board = ScoreBoard::load(path.clone());
        board.record("100.64.0.2", Outcome::Win, 10);
        board.record("100.64.0.2", Outcome::Win, 20);
        board.record("100.64.0.2", Outcome::Loss, 30);
        board.record("100.64.0.2", Outcome::Win, 40);

        let snapshot = board.snapshot();
        let record = &snapshot.records[0];
        assert_eq!(record.opponent, "100.64.0.2");
        assert_eq!(record.entry.wins, 3);
        assert_eq!(record.entry.losses, 1);
        assert_eq!(record.entry.games, 4);
        assert_eq!(record.entry.current_streak, 1);
        assert_eq!(record.entry.best_streak, 2);
        assert_eq!(record.entry.last_played_ms, 40);
        assert_eq!(snapshot.totals.games, 4);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn totals_merge_across_opponents() {
        let path = temp_path("totals");
        let board = ScoreBoard::load(path.clone());
        board.record("a", Outcome::Win, 1);
        board.record("b", Outcome::Loss, 2);
        board.record("b", Outcome::Draw, 3);

        let totals = board.snapshot().totals;
        assert_eq!(totals.wins, 1);
        assert_eq!(totals.losses, 1);
        assert_eq!(totals.draws, 1);
        assert_eq!(totals.games, 3);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn persists_across_reload() {
        let path = temp_path("persist");
        {
            let board = ScoreBoard::load(path.clone());
            board.record("100.64.0.2", Outcome::Win, 1);
        }
        let reloaded = ScoreBoard::load(path.clone());
        assert_eq!(reloaded.snapshot().records[0].entry.wins, 1);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn reset_clears_history() {
        let path = temp_path("reset");
        let board = ScoreBoard::load(path.clone());
        board.record("a", Outcome::Win, 1);
        let snapshot = board.reset();
        assert!(snapshot.records.is_empty());
        assert_eq!(snapshot.totals.games, 0);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn ignores_empty_opponent() {
        let path = temp_path("empty");
        let board = ScoreBoard::load(path.clone());
        board.record("", Outcome::Win, 1);
        assert!(board.snapshot().records.is_empty());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn counter_counts_side_zero_win() {
        let mut counter = ScoreCounter::new(0);
        assert!(counter.observe(Some(0), Some(0)).is_empty());
        assert_eq!(counter.observe(Some(1), Some(0)), vec![Outcome::Win]);
    }

    #[test]
    fn counter_counts_side_one_win_as_local_win() {
        let mut counter = ScoreCounter::new(1);
        counter.observe(Some(0), Some(0));
        assert_eq!(counter.observe(Some(0), Some(1)), vec![Outcome::Win]);
    }

    #[test]
    fn counter_counts_opponent_win_as_loss() {
        let mut counter = ScoreCounter::new(0);
        counter.observe(Some(0), Some(0));
        assert_eq!(counter.observe(Some(0), Some(1)), vec![Outcome::Loss]);
    }

    #[test]
    fn counter_counts_consecutive_games() {
        let mut counter = ScoreCounter::new(0);
        counter.observe(Some(0), Some(0));
        assert_eq!(counter.observe(Some(1), Some(0)), vec![Outcome::Win]);
        assert_eq!(counter.observe(Some(2), Some(0)), vec![Outcome::Win]);
        assert_eq!(counter.observe(Some(2), Some(1)), vec![Outcome::Loss]);
    }

    #[test]
    fn counter_ignores_reset_and_unparsable_scores() {
        let mut counter = ScoreCounter::new(0);
        counter.observe(Some(0), Some(0));
        counter.observe(Some(2), Some(1));
        assert!(counter.observe(Some(0), Some(0)).is_empty());
        assert!(counter.observe(None, Some(1)).is_empty());
        assert!(counter.observe(Some(1), None).is_empty());
    }

    #[test]
    fn counter_skips_ambiguous_multi_game_jumps() {
        let mut counter = ScoreCounter::new(0);
        counter.observe(Some(0), Some(0));
        assert!(counter.observe(Some(3), Some(1)).is_empty());
        assert_eq!(counter.observe(Some(4), Some(1)), vec![Outcome::Win]);
    }
}
