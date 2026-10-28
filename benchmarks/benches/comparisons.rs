use criterion::criterion_main;

mod cases;
mod fixed;
mod scaling;
mod selectors;
mod sizes;

criterion_main!(fixed::fixed, scaling::scaling);
