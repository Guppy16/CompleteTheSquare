//! The winning squares: 30 along the rows and columns, 50 with the
//! rotated-squares variant. Run both: cargo test --release, and again with
//! --features rotated-squares.

use complete_the_square_ai::game::{square_bit, tables};

/// "B1" -> its bit (columns A-E, rows 1-5 from the top).
fn bit(name: &str) -> u32 {
    let col = name.as_bytes()[0] - b'A';
    let row = name.as_bytes()[1] - b'1';
    square_bit(row as usize, col as usize)
}

fn mask(corners: &str) -> u32 {
    corners.split(' ').map(bit).fold(0, |m, b| m | b)
}

#[test]
fn counts_the_squares() {
    let expected = if cfg!(feature = "rotated-squares") { 50 } else { 30 };
    assert_eq!(tables().all_corner_masks.len(), expected);
}

#[test]
fn tilted_squares_win_only_in_the_variant() {
    let masks = &tables().all_corner_masks;
    assert!(masks.contains(&mask("A1 B1 A2 B2")));
    assert!(masks.contains(&mask("A1 E1 A5 E5")));
    for tilted in ["A2 B1 C2 B3", "A2 B5 E4 D1", "A4 C5 D3 B2"] {
        assert_eq!(masks.contains(&mask(tilted)), cfg!(feature = "rotated-squares"), "{tilted}");
    }
}
