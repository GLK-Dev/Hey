// Copyright (c) Vitaly Golik (mjojo)

//! Generates the screenshots shown on the project site with `Ui::render_png`, without a window.
//! `HEY_UPDATE_GOLDEN=1 cargo test -p hey-test --test site_screenshots` rewrites `site/assets/`;
//! a plain run fails if a committed screenshot no longer matches what the widgets draw.

use hey_core::{Callback, Insets, Size};
use hey_layout::Style;
use hey_render_cpu::Framebuffer;
use hey_style::{Theme, Variant};
use hey_test::Harness;
use hey_widgets::{Button, Checkbox, Label, ProgressBar, Slider, TextInput, Toggle};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../site/assets");

fn scene(theme: Theme) -> Harness<String> {
    let mut h = Harness::<String>::with_theme(theme, 480.0, 300.0);
    let root = h.ui.root();
    let col = h.ui.add_container(root, Style::column().padding(Insets::all(24.0)).gap(14.0));
    Label::add(&mut h.ui, col, "Hey: native UI in Rust");
    let row = h.ui.add_container(col, Style::row().gap(8.0));
    Button::add(&mut h.ui, row, "Primary", Variant::Primary, Callback::new(|_| String::new()));
    Button::add(&mut h.ui, row, "Secondary", Variant::Secondary, Callback::new(|_| String::new()));
    Button::add(&mut h.ui, row, "Danger", Variant::Danger, Callback::new(|_| String::new()));
    TextInput::add(&mut h.ui, col, "Deterministic", 300.0, Callback::new(|s| s));
    Checkbox::add(&mut h.ui, col, "Checkbox", true, Callback::new(|_| String::new()));
    Toggle::add(&mut h.ui, col, "Toggle", true, Callback::new(|_| String::new()));
    Slider::add(&mut h.ui, col, 0.0, 100.0, 40.0, 300.0, Callback::new(|_| String::new()));
    ProgressBar::add(&mut h.ui, col, 0.6, 300.0);
    h.frame();
    h
}

fn check(name: &str, bytes: Vec<u8>) {
    let path = std::path::Path::new(ASSETS).join(format!("{name}.png"));
    if std::env::var_os("HEY_UPDATE_GOLDEN").is_some() || !path.exists() {
        std::fs::write(&path, &bytes).unwrap();
        return;
    }
    let want = Framebuffer::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    let got = Framebuffer::decode_png(&bytes).unwrap();
    assert_eq!((want.width, want.height), (got.width, got.height), "{name}: size");
    assert!(want.data == got.data, "{name}: differs from site/assets (HEY_UPDATE_GOLDEN=1 to update)");
}

#[test]
fn site_screenshots_match_the_widgets() {
    check("screenshot-dark", scene(Theme::dark()).ui.render_png(Size::new(480.0, 300.0)));
    check("screenshot-light", scene(Theme::light()).ui.render_png(Size::new(480.0, 300.0)));
}

#[test]
fn render_png_is_deterministic_and_scales() {
    let mut h = scene(Theme::dark());
    let a = h.ui.render_png(Size::new(480.0, 300.0));
    assert_eq!(&a[..8], b"\x89PNG\r\n\x1a\n");
    assert!(a == h.ui.render_png(Size::new(480.0, 300.0)));
    h.set_scale(2.0);
    let fb = h.ui.render(Size::new(960.0, 600.0));
    assert_eq!((fb.width, fb.height), (960, 600));
    assert!(Framebuffer::decode_png(&a).unwrap().data.len() == 480 * 300 * 4);
}
