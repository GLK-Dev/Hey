// Copyright (c) Vitaly Golik (mjojo)

//! Accessibility lints over an [`A11yTree`]: structural checks that need no screen reader.

use hey_widgets::{A11yNode, A11yTree, Role};
use std::fmt;

/// Minimum interactive target size in logical pixels (WCAG 2.2 AA, 2.5.8).
pub const MIN_TARGET: f32 = 24.0;

/// What a lint found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IssueKind {
    /// An interactive control has no accessible name.
    Unnamed,
    /// An interactive control is smaller than [`MIN_TARGET`] in either direction.
    SmallTarget,
    /// An interactive control cannot be reached by keyboard focus.
    NotFocusable,
}

/// One finding, tied to a node.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub kind: IssueKind,
    /// `A11yNode::id` of the offending node.
    pub node: u64,
    pub role: Role,
    pub label: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            IssueKind::Unnamed => "has no accessible name",
            IssueKind::SmallTarget => "is smaller than 24x24 logical px",
            IssueKind::NotFocusable => "cannot be reached by keyboard",
        };
        write!(f, "{:?} \"{}\" (node {}) {what}", self.role, self.label, self.node)
    }
}

fn interactive(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::CheckBox
            | Role::Switch
            | Role::RadioButton
            | Role::Slider
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::ComboBox
    )
}

/// Runs every lint on a tree in logical pixels (layout units). Disabled controls are skipped: they take no input.
pub fn audit_tree(tree: &A11yTree) -> Vec<Issue> {
    let mut out = Vec::new();
    for n in tree.nodes.iter().filter(|n| interactive(n.a11y.role) && !n.disabled) {
        let mut push = |kind| out.push(issue(kind, n));
        if n.a11y.label.trim().is_empty() {
            push(IssueKind::Unnamed);
        }
        if n.rect.w < MIN_TARGET || n.rect.h < MIN_TARGET {
            push(IssueKind::SmallTarget);
        }
        if !n.focusable {
            push(IssueKind::NotFocusable);
        }
    }
    out
}

fn issue(kind: IssueKind, n: &A11yNode) -> Issue {
    Issue { kind, node: n.id, role: n.a11y.role, label: n.a11y.label.clone() }
}
