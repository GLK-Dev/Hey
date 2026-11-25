// Copyright (c) Vitaly Golik (mjojo)

//! Layout benchmarks: a 10k-node tree, full relayout and hit-test.

use criterion::{criterion_group, criterion_main, Criterion};
use hey_core::{Point, Size};
use hey_layout::{LayoutTree, Style};
use std::hint::black_box;

/// 100 rows x 100 leaves + 100 row nodes + root = 10 101 nodes.
fn build() -> LayoutTree {
    let mut t = LayoutTree::new(Style::column().gap(2.0));
    for _ in 0..100 {
        let row = t.add(t.root(), Style::row().gap(2.0).height(20.0)).unwrap();
        for i in 0..100 {
            t.add(row, Style::leaf().size(10.0 + (i % 5) as f32, 16.0)).unwrap();
        }
    }
    t
}

fn bench(c: &mut Criterion) {
    let vp = Size::new(1920.0, 4000.0);
    c.bench_function("layout_10k_full", |b| {
        let mut t = build();
        b.iter(|| {
            t.invalidate();
            t.compute(black_box(vp));
        });
    });
    c.bench_function("layout_10k_cached", |b| {
        let mut t = build();
        t.compute(vp);
        b.iter(|| t.compute(black_box(vp)));
    });
    c.bench_function("hit_test_10k", |b| {
        let mut t = build();
        t.compute(vp);
        b.iter(|| t.hit_test(black_box(Point::new(900.0, 1500.0))));
    });
    c.bench_function("build_10k", |b| b.iter(build));
}

criterion_group!(benches, bench);
criterion_main!(benches);
