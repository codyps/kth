use super::{cases, inputs};
use criterion::{criterion_group, Criterion};
#[path = "support/page_cases.rs"]
mod page_cases;

fn suite(c: &mut Criterion) {
    // Tests can exercise every adapter on small inputs without running the
    // published large-input matrix. The report records any explicit override.
    let override_sizes = std::env::var("KTH_PAGE_SIZES").ok().map(|values| {
        let mut sizes: Vec<usize> = values.split(',').map(|n| n.parse().unwrap()).collect();
        assert!(!sizes.is_empty() && sizes.iter().all(|n| *n > 0));
        sizes.sort_unstable();
        sizes.dedup();
        sizes
    });
    let mut seen = std::collections::BTreeSet::new();
    for case in page_cases::cases() {
        for &n in override_sizes.as_ref().unwrap_or(&case.sizes) {
            for &rank in &case.ranks {
                if !seen.insert((case.kind, case.distribution, rank, n)) {
                    continue;
                }
                let k = page_cases::rank(rank, n);
                let mut group = c.benchmark_group(format!(
                    "coverage-partition/{}/{}/{rank}",
                    case.kind, case.distribution
                ));
                macro_rules! numeric {
                    ($convert:expr) => {{
                        let bank = page_cases::bank(&case, n, $convert);
                        cases::partition_inputs_at(&mut group, &bank, true, k);
                    }};
                }
                match case.kind {
                    "u8" => {
                        let bank = page_cases::bank(&case, n, |v| v as u8);
                        cases::partition_inputs_at(&mut group, &bank, true, k);
                        cases::bench_partition_inputs_at(
                            &mut group,
                            "kth-u8",
                            &bank,
                            true,
                            k,
                            kth::partition_u8_by_kth,
                        );
                    }
                    "u64" => numeric!(|v| v),
                    "17-byte" => numeric!(page_cases::bytes::<17>),
                    "32-byte" => numeric!(page_cases::bytes::<32>),
                    "33-byte" => numeric!(page_cases::bytes::<33>),
                    "64-byte" => numeric!(|v| [v; 8]),
                    "128-byte" => numeric!(|v| [v; 16]),
                    "String" | "str" => {
                        let words = inputs::strings(case.distribution, &inputs::random_bank(n));
                        if case.kind == "String" {
                            cases::owned_partitions_at(&mut group, &words, true, k);
                        } else {
                            let borrowed: Vec<Vec<_>> = words
                                .iter()
                                .map(|a| a.iter().map(String::as_str).collect())
                                .collect();
                            cases::partition_inputs_at(&mut group, &borrowed, true, k);
                        }
                    }
                    _ => panic!("unknown page type: {}", case.kind),
                }
                group.finish();
            }
        }
    }
}

criterion_group!(coverage, suite);
