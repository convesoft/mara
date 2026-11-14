//! Typo tolerance (REQ-TYPO-TOLERANT-SEARCH in `docs/retrieval.mara.md`).

/// Whether `a` and `b` are at most `max` edits apart. An edit inserts, deletes or substitutes
/// one character, or swaps two adjacent characters: the optimal string alignment distance, the
/// restricted form of the Damerau–Levenshtein distance. Only the cells within `max` of the
/// diagonal are computed, so the work grows with the length times `max`.
pub(crate) fn within(a: &[char], b: &[char], max: usize) -> bool {
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max {
        return false;
    }
    // A cell beyond `max` edits counts as `far`; one row of the table per buffer, three rows
    // kept for the swap of two characters.
    let far = max + 1;
    let mut before = vec![far; m + 1];
    let mut previous: Vec<usize> = (0..=m).map(|j| j.min(far)).collect();
    let mut current = vec![far; m + 1];
    for i in 1..=n {
        let low = i.saturating_sub(max).max(1);
        let high = (i + max).min(m);
        // The cell left of the band is outside it, except in column 0.
        current[low - 1] = if low == 1 { i.min(far) } else { far };
        let mut best = current[low - 1];
        for j in low..=high {
            let substitution = previous[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            let mut cell = substitution.min(previous[j] + 1).min(current[j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                cell = cell.min(before[j - 2] + 1);
            }
            current[j] = cell.min(far);
            best = best.min(current[j]);
        }
        if high < m {
            current[high + 1] = far;
        }
        if best > max {
            return false;
        }
        std::mem::swap(&mut before, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous[m] <= max
}
