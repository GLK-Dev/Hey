use hey_core::{Insets, Rect, Size};
use hey_layout::{Align, Justify, LayoutTree, Style};

fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, w, h)
}

#[test]
fn row_gap_padding_margin() {
    let mut t = LayoutTree::new(Style::row().gap(10.0).padding(Insets::all(5.0)));
    let a = t.add(t.root(), Style::leaf().size(20.0, 10.0)).unwrap();
    let b = t.add(t.root(), Style::leaf().size(30.0, 10.0).margin(Insets::symmetric(2.0, 3.0))).unwrap();
    t.compute(Size::new(200.0, 100.0));
    assert_eq!(t.rect(a), r(5.0, 5.0, 20.0, 10.0)); // явная высота не растягивается
    assert_eq!(t.rect(b), r(5.0 + 20.0 + 10.0 + 2.0, 5.0 + 3.0, 30.0, 10.0));
}

#[test]
fn grow_shares_free_space_and_spacer() {
    let mut t = LayoutTree::new(Style::row());
    let a = t.add(t.root(), Style::leaf().width(50.0)).unwrap();
    let s = t.add(t.root(), Style::spacer()).unwrap();
    let b = t.add(t.root(), Style::leaf().width(50.0).grow(1.0)).unwrap();
    t.compute(Size::new(300.0, 40.0));
    assert_eq!(t.rect(a).w, 50.0);
    assert_eq!(t.rect(s), r(50.0, 0.0, 100.0, 40.0));
    assert_eq!(t.rect(b), r(150.0, 0.0, 150.0, 40.0));
}

#[test]
fn shrink_and_min() {
    let mut t = LayoutTree::new(Style::row());
    let a = t.add(t.root(), Style::leaf().width(200.0)).unwrap();
    let b = t.add(t.root(), Style::leaf().width(200.0).min_size(150.0, 0.0)).unwrap();
    let c = t.add(t.root(), Style::leaf().width(200.0).shrink(0.0)).unwrap();
    t.compute(Size::new(500.0, 10.0));
    assert_eq!(t.rect(c).w, 200.0);
    assert_eq!(t.rect(a).w + t.rect(b).w, 300.0);
    assert!(t.rect(b).w >= 150.0);
}

#[test]
fn grow_respects_max() {
    let mut t = LayoutTree::new(Style::row());
    let a = t.add(t.root(), Style::leaf().grow(1.0).max_size(40.0, f32::INFINITY)).unwrap();
    t.compute(Size::new(300.0, 10.0));
    assert_eq!(t.rect(a).w, 40.0);
}

#[test]
fn justify_modes() {
    let cases = [
        (Justify::Start, [0.0, 30.0]),
        (Justify::Center, [70.0, 100.0]),
        (Justify::End, [140.0, 170.0]),
        (Justify::SpaceBetween, [0.0, 170.0]),
        (Justify::SpaceAround, [35.0, 135.0]),
        (Justify::SpaceEvenly, [47.0, 123.0]),
    ];
    for (j, xs) in cases {
        let mut t = LayoutTree::new(Style::row().justify(j));
        let a = t.add(t.root(), Style::leaf().size(30.0, 10.0)).unwrap();
        let b = t.add(t.root(), Style::leaf().size(30.0, 10.0)).unwrap();
        t.compute(Size::new(200.0, 10.0));
        assert_eq!([t.rect(a).x, t.rect(b).x], xs, "{j:?}");
    }
}

#[test]
fn cross_alignment_column() {
    let mut t = LayoutTree::new(Style::column().align_items(Align::Center).gap(4.0));
    let a = t.add(t.root(), Style::leaf().size(40.0, 20.0)).unwrap();
    let b = t.add(t.root(), Style::leaf().size(60.0, 20.0).align_self(Align::End)).unwrap();
    t.compute(Size::new(100.0, 100.0));
    assert_eq!(t.rect(a), r(30.0, 0.0, 40.0, 20.0));
    assert_eq!(t.rect(b), r(40.0, 24.0, 60.0, 20.0));
}

#[test]
fn stack_overlays_and_aligns() {
    let mut t = LayoutTree::new(Style::stack().padding(Insets::all(10.0)));
    let bg = t.add(t.root(), Style::leaf()).unwrap();
    let badge = t.add(t.root(), Style::leaf().size(20.0, 20.0).align_self(Align::End)).unwrap();
    t.compute(Size::new(100.0, 80.0));
    assert_eq!(t.rect(bg), r(10.0, 10.0, 80.0, 60.0));
    assert_eq!(t.rect(badge), r(70.0, 50.0, 20.0, 20.0));
}

#[test]
fn nested_auto_sizes_from_content() {
    let mut t = LayoutTree::new(Style::column().align_items(Align::Start));
    let row = t.add(t.root(), Style::row().gap(5.0).padding(Insets::all(2.0))).unwrap();
    let a = t.add(row, Style::leaf().size(10.0, 8.0)).unwrap();
    let b = t.add(row, Style::leaf().size(10.0, 12.0)).unwrap();
    t.compute(Size::new(100.0, 100.0));
    assert_eq!(t.rect(row), r(0.0, 0.0, 29.0, 16.0));
    assert_eq!(t.rect(a).x, 2.0);
    assert_eq!(t.rect(b).x, 17.0);
}

#[test]
fn text_wraps_by_available_width() {
    // Лист «текст»: 10 px на символ, перенос по ширине, высота 10 px на строку.
    let text_len = 30.0f32;
    let mut t = LayoutTree::new(Style::column().align_items(Align::Start));
    let p = t.add(t.root(), Style::leaf().width(100.0)).unwrap();
    t.set_measure(p, move |w| {
        let total = text_len * 10.0;
        match w {
            Some(w) => Size::new(w, (total / w).ceil() * 10.0),
            None => Size::new(total, 10.0),
        }
    });
    t.compute(Size::new(400.0, 400.0));
    assert_eq!(t.rect(p), r(0.0, 0.0, 100.0, 30.0));
}

#[test]
fn hidden_is_skipped_and_pixels_snap() {
    let mut t = LayoutTree::new(Style::row());
    let h = t.add(t.root(), Style::leaf().width(50.0)).unwrap();
    t.set_style(h, Style::leaf().width(50.0).hidden());
    let a = t.add(t.root(), Style::leaf().grow(1.0)).unwrap();
    let b = t.add(t.root(), Style::leaf().grow(1.0)).unwrap();
    let c = t.add(t.root(), Style::leaf().grow(1.0)).unwrap();
    t.compute(Size::new(100.0, 10.0));
    assert_eq!(t.rect(h), Rect::default());
    let ws = [t.rect(a).w, t.rect(b).w, t.rect(c).w];
    assert_eq!(ws.iter().sum::<f32>(), 100.0);
    assert!(ws.iter().all(|w| w.fract() == 0.0));
    assert_eq!(t.rect(b).x, t.rect(a).right());
}

mod hit {
    use super::*;
    use hey_core::Point;

    #[test]
    fn z_order_and_pass_through() {
        let mut t = LayoutTree::new(Style::stack());
        let a = t.add(t.root(), Style::leaf().size(50.0, 50.0).align_self(Align::Start)).unwrap();
        let b = t.add(t.root(), Style::leaf().size(50.0, 50.0).align_self(Align::Start)).unwrap();
        t.compute(Size::new(100.0, 100.0));
        assert_eq!(t.hit_test(Point::new(10.0, 10.0)), Some(b)); // позже — выше
        t.set_style(a, Style::leaf().size(50.0, 50.0).align_self(Align::Start).z(5));
        assert_eq!(t.hit_test(Point::new(10.0, 10.0)), Some(a));
        t.set_style(a, Style::leaf().size(50.0, 50.0).align_self(Align::Start).z(5).pass_through());
        assert_eq!(t.hit_test(Point::new(10.0, 10.0)), Some(b));
        assert_eq!(t.hit_test(Point::new(90.0, 90.0)), Some(t.root()));
    }

    #[test]
    fn scroll_moves_content_and_clips() {
        let mut t = LayoutTree::new(Style::column());
        let sc = t.add(t.root(), Style::column().height(100.0).scroll(false, true)).unwrap();
        let items: Vec<_> = (0..5).map(|_| t.add(sc, Style::leaf().height(60.0)).unwrap()).collect();
        let below = t.add(t.root(), Style::leaf().height(50.0)).unwrap();
        t.compute(Size::new(100.0, 300.0));
        assert_eq!(t.max_scroll(sc).y, 200.0);
        assert_eq!(t.hit_test(Point::new(5.0, 70.0)), Some(items[1]));
        // Элемент за границей области скролла не ловится, даже если геометрически лежит под соседом.
        assert_eq!(t.hit_test(Point::new(5.0, 110.0)), Some(below));
        assert!(t.scroll_by(sc, 0.0, 70.0));
        t.compute(Size::new(100.0, 300.0));
        assert_eq!(t.rect(items[1]).y, -10.0);
        assert_eq!(t.hit_test(Point::new(5.0, 5.0)), Some(items[1]));
        t.scroll_by(sc, 0.0, 1000.0);
        assert_eq!(t.scroll(sc).y, 200.0);
        assert!(!t.scroll_by(sc, 0.0, 5.0));
    }

    #[test]
    fn popup_escapes_clip_and_wins() {
        let mut t = LayoutTree::new(Style::column());
        let clipped = t.add(t.root(), Style::stack().size(50.0, 50.0).clip()).unwrap();
        let pop = t.add(clipped, Style::leaf().size(80.0, 80.0).align_self(Align::Start).popup()).unwrap();
        let other = t.add(t.root(), Style::leaf().height(100.0)).unwrap();
        t.compute(Size::new(200.0, 200.0));
        assert_eq!(t.hit_test(Point::new(70.0, 70.0)), Some(pop)); // вне clip, но попап
        assert_eq!(t.hit_test(Point::new(150.0, 150.0)), Some(t.root()));
        let _ = other;
    }
}
