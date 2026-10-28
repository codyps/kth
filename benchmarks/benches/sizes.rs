pub fn sizes() -> Vec<usize> {
    let values = std::env::var("KTH_BENCH_SIZES")
        .unwrap_or_else(|_| "16,64,256,1024,4096,16384,65536".into());
    let mut sizes: Vec<usize> = values
        .split(',')
        .map(|value| {
            value
                .parse()
                .expect("KTH_BENCH_SIZES must contain integers")
        })
        .collect();
    assert!(sizes.iter().all(|&n| n > 0), "sizes must be positive");
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}
