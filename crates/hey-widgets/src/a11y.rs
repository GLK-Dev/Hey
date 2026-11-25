// Copyright (c) Vitaly Golik (mjojo)

//! Accessibility model: a toolkit-neutral description of the UI that a platform adapter
//! (accesskit in `hey-shell`) turns into the OS accessibility tree.

use hey_core::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Generic,
    Label,
    Image,
    Button,
    CheckBox,
    Switch,
    RadioButton,
    Slider,
    ProgressBar,
    TextInput,
    MultilineTextInput,
    List,
    TabList,
    Menu,
    ComboBox,
    Tooltip,
    Dialog,
}

/// What a widget exposes to assistive technology.
#[derive(Clone, Debug, PartialEq)]
pub struct A11y {
    pub role: Role,
    pub label: String,
    pub value: Option<String>,
    /// Checked / on state for checkboxes, switches and radio buttons.
    pub toggled: Option<bool>,
    /// `(min, max, current)` for sliders and progress bars.
    pub range: Option<(f64, f64, f64)>,
}

impl A11y {
    pub fn new(role: Role, label: impl Into<String>) -> Self {
        A11y { role, label: label.into(), value: None, toggled: None, range: None }
    }
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = Some(v.into());
        self
    }
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = Some(on);
        self
    }
    pub fn range(mut self, min: f64, max: f64, now: f64) -> Self {
        self.range = Some((min, max, now));
        self
    }
}

/// One node of the accessibility tree. `id` is stable while the widget lives.
#[derive(Clone, Debug)]
pub struct A11yNode {
    pub id: u64,
    pub rect: Rect,
    pub a11y: A11y,
    pub disabled: bool,
    pub focusable: bool,
    pub children: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct A11yTree {
    pub root: u64,
    pub nodes: Vec<A11yNode>,
    pub focus: Option<u64>,
}

/// Request from assistive technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum A11yAction {
    Focus(u64),
    /// Default action (click / press).
    Activate(u64),
}
