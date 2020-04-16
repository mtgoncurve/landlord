//! # Maximum bipartite matching implementation

// Various resources for my own benefit
// - https://github.com/mtgoncurve/landlord/issues/16
// - https://www.youtube.com/watch?v=HZLKDC9OSaQ
// - https://ocw.mit.edu/courses/electrical-engineering-and-computer-science/6-042j-mathematics-for-computer-science-fall-2010/readings/MIT6_042JF10_chap05.pdf
// - https://en.wikipedia.org/wiki/Ford%E2%80%93Fulkerson_algorithm
// - https://en.wikipedia.org/wiki/Hopcroft%E2%80%93Karp_algorithm
// - https://en.wikipedia.org/wiki/Edmonds%E2%80%93Karp_algorithm
// - http://olympiad.cs.uct.ac.za/presentations/camp2_2017/bipartitematching-robin.pdf

/// Returns the size of the maximum matching set of the
/// bipartite graph represented by the adjacency matrix
/// `edges` with `m_count` rows and `n_count` columns.
/// `seen` and `matches` are implementation-specific data structures
/// that are expected to be correctly sized by the caller to reduce
/// runtime allocations.
/// Implementation based on the "Alternate Approach" from
/// http://olympiad.cs.uct.ac.za/presentations/camp2_2017/bipartitematching-robin.pdf
pub fn maximum_bipartite_matching(
    edges: &Vec<u8>,
    m_count: usize,
    n_count: usize,
    seen: &mut Vec<bool>,
    matches: &mut Vec<i32>,
) -> usize {
    let mut match_count = 0;
    // reset matches
    for mat in matches.iter_mut() {
        *mat = -1;
    }
    // for each mana pip
    for m in 0..m_count {
        // reset lands seen
        for s in seen.iter_mut() {
            *s = false;
        }
        // Attempt to find a matching land
        let found_match = recursive_find_match(edges, m_count, n_count, m, seen, matches);
        if found_match {
            match_count += 1;
        }
    }
    match_count
}

fn recursive_find_match(
    edges: &Vec<u8>,
    m_count: usize,
    n_count: usize,
    m: usize,
    seen: &mut Vec<bool>,
    matches: &mut Vec<i32>,
) -> bool {
    // for each land
    for n in 0..n_count {
        let i = n_count * m + n;
        // Is this the first time we're seeing this land and does this land pay for pip m?
        if edges[i] != 0 && !seen[n] {
            seen[n] = true;
            // Is this land available to tap OR can we find a different land for pip (matches[n]) that
            // previously matched with this land
            let this_land_or_other_land_available = matches[n] < 0
                || recursive_find_match(
                    edges,
                    m_count,
                    n_count,
                    matches[n] as usize,
                    seen,
                    matches,
                );
            if this_land_or_other_land_available {
                matches[n] = m as i32;
                return true;
            }
        }
    }
    false
}
