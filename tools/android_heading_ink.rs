#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadingInkHeights {
    pub first_h1: usize,
    pub first_h2: usize,
    pub first_body: usize,
    pub second_h1: usize,
    pub second_h2: usize,
    pub second_body: usize,
}

/// Finds the authored H1/H2/body/H1/H2/body sequence in top-to-bottom ink bands.
/// It deliberately does not assume how many lines the preceding paragraph wraps to.
pub fn select_heading_ink_sequence(heights: &[usize]) -> Result<HeadingInkHeights, String> {
    let mut found = None;
    for sample in heights.windows(6) {
        let candidate = HeadingInkHeights {
            first_h1: sample[0],
            first_h2: sample[1],
            first_body: sample[2],
            second_h1: sample[3],
            second_h2: sample[4],
            second_body: sample[5],
        };
        let hierarchy = candidate.first_h1 > candidate.first_h2
            && candidate.first_h2 > candidate.first_body
            && candidate.second_h1 > candidate.second_h2
            && candidate.second_h2 > candidate.second_body;
        let stable = candidate.first_h1.abs_diff(candidate.second_h1) <= 5
            && candidate.first_h2.abs_diff(candidate.second_h2) <= 5
            && candidate.first_body.abs_diff(candidate.second_body) <= 5;
        if hierarchy && stable {
            if found.replace(candidate).is_some() {
                return Err("Ambiguous contiguous H1/H2/body/H1/H2/body ink sequence".into());
            }
        }
    }
    found.ok_or_else(|| "No contiguous H1/H2/body/H1/H2/body ink sequence with stable level sizes".into())
}

#[cfg(test)]
mod tests {
    use super::{select_heading_ink_sequence, HeadingInkHeights};

    // Measured from the real API 35/36 share PNGs: the pre-existing paragraph
    // wraps to two ink rows, shifting the six sample rows by one band.
    const HISTORICAL_BANDS: [usize; 12] = [58, 25, 37, 24, 24, 37, 30, 24, 37, 30, 24, 22];

    #[test]
    fn historical_wrapped_paragraph_does_not_shift_sample_detection() {
        let sample = select_heading_ink_sequence(&HISTORICAL_BANDS).unwrap();
        assert_eq!(sample, HeadingInkHeights {
            first_h1: 37, first_h2: 30, first_body: 24,
            second_h1: 37, second_h2: 30, second_body: 24,
        });
    }

    #[test]
    fn typography_changes_preserve_the_hierarchy_contract() {
        let bands = [58, 25, 39, 26, 26, 39, 32, 26, 39, 32, 26, 22];
        assert!(select_heading_ink_sequence(&bands).is_ok());
    }

    #[test]
    fn reversed_heading_levels_are_rejected() {
        let bands = [58, 25, 37, 24, 24, 37, 30, 24, 30, 37, 24, 22];
        assert!(select_heading_ink_sequence(&bands).is_err());
    }
}
