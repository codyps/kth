use criterion::criterion_main;

mod cases;
mod coverage;
mod fixed;
mod inputs;
mod scaling;
mod selectors;
mod sizes;

criterion_main!(fixed::fixed, scaling::scaling, coverage::coverage);
