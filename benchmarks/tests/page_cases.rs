#[allow(dead_code)]
#[path = "../benches/inputs.rs"]
mod inputs;
#[path = "../benches/support/page_cases.rs"]
mod page_cases;

#[test]
fn published_matrix_has_unique_valid_cases_and_wide_threshold_coverage() {
    let mut keys = std::collections::BTreeSet::new();
    for case in page_cases::cases() {
        assert!(!case.ranks.is_empty() && !case.sizes.is_empty());
        for n in case.sizes {
            for &rank in &case.ranks {
                assert!(page_cases::rank(rank, n) < n);
                assert!(keys.insert((case.kind, case.distribution, rank, n)));
            }
        }
    }
    for kind in ["64-byte", "128-byte"] {
        for n in [16383, 16384, 16385, 1048576] {
            assert!(keys.contains(&(kind, "random", "median", n)));
        }
        for rank in ["p01", "p25", "median", "p75", "p99"] {
            assert!(keys.contains(&(kind, "random", rank, 65536)));
        }
    }
    assert!(keys.contains(&("33-byte", "periodic17", "median", 16384)));
    assert!(keys.contains(&("33-byte", "random17", "median", 16384)));
}

#[test]
fn ordered_byte_cases_are_monotone_after_wrapping_and_periodic_cases_are_not_sorted() {
    let cases = page_cases::cases();
    for distribution in ["ordered-asc", "ordered-desc"] {
        let case = cases
            .iter()
            .find(|c| c.kind == "u8" && c.distribution == distribution)
            .unwrap();
        for n in [257, 1024, 16384] {
            let bank = page_cases::bank(case, n, |v| v as u8);
            assert_eq!(bank.len(), 1);
            assert!(bank[0]
                .windows(2)
                .all(|w| if distribution == "ordered-asc" {
                    w[0] <= w[1]
                } else {
                    w[0] >= w[1]
                }));
        }
    }
    let case = cases
        .iter()
        .find(|c| c.distribution == "periodic17")
        .unwrap();
    let bank = page_cases::bank(case, 1024, page_cases::bytes::<33>);
    assert_eq!(bank[0][0], bank[0][17]);
    assert!(bank[0].windows(2).any(|w| w[0] > w[1]));
}

#[test]
fn random_banks_are_distinct_reproducible_and_low_cardinality_stays_bounded() {
    let cases = page_cases::cases();
    for distribution in ["random", "random17"] {
        let case = cases
            .iter()
            .find(|c| c.kind == "33-byte" && c.distribution == distribution)
            .unwrap();
        let bank = page_cases::bank(case, 257, |v| v);
        assert_eq!(bank.len(), 16);
        assert_ne!(bank[0], bank[1]);
        assert_eq!(bank, page_cases::bank(case, 257, |v| v));
        if distribution == "random17" {
            assert!(bank.iter().flatten().all(|v| *v < 17));
        }
    }
}
