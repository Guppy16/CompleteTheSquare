//! The Rust engine must give exactly the Python engine's answers.
//! `gen_parity.py` produces random positions and the Python answers; this
//! test runs it (needs `python3` on PATH) and replays every case in Rust.

use complete_the_square_ai::game::{play_move, square_bit, tables, State};
use complete_the_square_ai::search::{best_move_index, evaluator};

const DEPTH: u32 = 4;

#[test]
fn matches_python_engine() {
    let output = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/gen_parity.py"))
        .output()
        .expect("python3 is needed to generate the parity cases");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    let (t, ev) = (tables(), evaluator());
    let mut count = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let state = State { boards: [f[0].parse().unwrap(), f[1].parse().unwrap()], current: f[2].parse().unwrap() };
        let (mr, mc): (usize, usize) = (f[3].parse().unwrap(), f[4].parse().unwrap());
        let expected = State { boards: [f[5].parse().unwrap(), f[6].parse().unwrap()], current: f[7].parse().unwrap() };
        let expected_won = f[8] == "1";
        let expected_eval: f64 = f[9].parse().unwrap();
        let expected_best: (usize, usize) = (f[10].parse().unwrap(), f[11].parse().unwrap());

        let (next, won) = play_move(t, square_bit(mr, mc), &state);
        assert_eq!(next, expected, "play_move mismatch: {line}");
        assert_eq!(won.is_some(), expected_won, "win mismatch: {line}");

        let eval = ev.evaluate(t, &state);
        assert!((eval - expected_eval).abs() < 1e-12, "eval mismatch: {line} got {eval}");

        let best = best_move_index(&state, DEPTH).unwrap();
        assert_eq!((best / 5, best % 5), expected_best, "best move mismatch: {line}");
        count += 1;
    }
    assert!(count >= 100, "fixture too small");
}
