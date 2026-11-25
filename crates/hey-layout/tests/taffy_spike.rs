//! ADR-7 spike: the same scenarios as in layout.rs, on taffy — comparing the results.
use hey_core::Size;
use hey_layout::{LayoutTree, Style};
use taffy::prelude::*;

#[test]
fn row_with_grow_matches_taffy() {
    // Our engine: Row 300x100, gap 10, children: 50 fixed, grow 1, grow 2.
    let mut t = LayoutTree::new(Style::row().gap(10.0));
    let a = t.add(t.root(), Style::leaf().width(50.0)).unwrap();
    let b = t.add(t.root(), Style::leaf().grow(1.0)).unwrap();
    let c = t.add(t.root(), Style::leaf().grow(2.0)).unwrap();
    t.compute(Size::new(300.0, 100.0));

    let mut tf: TaffyTree<()> = TaffyTree::new();
    let na = tf.new_leaf(taffy::Style { size: taffy::Size { width: length(50.0), height: auto() }, ..Default::default() }).unwrap();
    let nb = tf.new_leaf(taffy::Style { flex_grow: 1.0, flex_basis: length(0.0), ..Default::default() }).unwrap();
    let nc = tf.new_leaf(taffy::Style { flex_grow: 2.0, flex_basis: length(0.0), ..Default::default() }).unwrap();
    let root = tf
        .new_with_children(
            taffy::Style {
                display: Display::Flex,
                gap: taffy::Size { width: length(10.0), height: length(0.0) },
                size: taffy::Size { width: length(300.0), height: length(100.0) },
                ..Default::default()
            },
            &[na, nb, nc],
        )
        .unwrap();
    tf.compute_layout(root, taffy::Size::MAX_CONTENT).unwrap();
    for (ours, theirs) in [(a, na), (b, nb), (c, nc)] {
        let r = t.rect(ours);
        let l = tf.layout(theirs).unwrap();
        eprintln!("ours {:?} taffy x={} y={} w={} h={}", r, l.location.x, l.location.y, l.size.width, l.size.height);
        assert!((r.x - l.location.x).abs() <= 1.0 && (r.w - l.size.width).abs() <= 1.0);
        assert!((r.h - l.size.height).abs() <= 1.0);
    }
}
