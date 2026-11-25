// Copyright (c) Vitaly Golik (mjojo)

//! Bridge from the neutral `hey-widgets` accessibility model to AccessKit.

use accesskit::{Action, Node, NodeId, Rect, Role as AkRole, TreeId, TreeInfo, TreeUpdate};
use hey_widgets::{A11yAction, A11yTree, Role};

/// Wraps AccessKit window events so they can travel through the winit event loop.
#[derive(Debug)]
pub enum UserEvent {
    Access(accesskit_winit::Event),
}

impl From<accesskit_winit::Event> for UserEvent {
    fn from(e: accesskit_winit::Event) -> Self {
        UserEvent::Access(e)
    }
}

fn role(r: Role) -> AkRole {
    match r {
        Role::Generic => AkRole::GenericContainer,
        Role::Label => AkRole::Label,
        Role::Image => AkRole::Image,
        Role::Button => AkRole::Button,
        Role::CheckBox => AkRole::CheckBox,
        Role::Switch => AkRole::Switch,
        Role::RadioButton => AkRole::RadioButton,
        Role::Slider => AkRole::Slider,
        Role::ProgressBar => AkRole::ProgressIndicator,
        Role::TextInput => AkRole::TextInput,
        Role::MultilineTextInput => AkRole::MultilineTextInput,
        Role::List => AkRole::List,
        Role::TabList => AkRole::TabList,
        Role::Menu => AkRole::Menu,
        Role::ComboBox => AkRole::ComboBox,
        Role::Tooltip => AkRole::Tooltip,
        Role::Dialog => AkRole::Dialog,
    }
}

/// Converts a full snapshot into an AccessKit tree update. The root becomes the window node.
pub fn to_update(tree: &A11yTree, title: &str) -> TreeUpdate {
    let nodes = tree
        .nodes
        .iter()
        .map(|n| {
            let is_root = n.id == tree.root;
            let mut node = Node::new(if is_root { AkRole::Window } else { role(n.a11y.role) });
            if is_root {
                node.set_label(title);
            } else if !n.a11y.label.is_empty() {
                node.set_label(n.a11y.label.as_str());
            }
            if let Some(v) = &n.a11y.value {
                node.set_value(v.as_str());
            }
            if let Some(t) = n.a11y.toggled {
                node.set_toggled(t.into());
            }
            if let Some((min, max, cur)) = n.a11y.range {
                node.set_min_numeric_value(min);
                node.set_max_numeric_value(max);
                node.set_numeric_value(cur);
            }
            node.set_bounds(Rect {
                x0: n.rect.x as f64,
                y0: n.rect.y as f64,
                x1: (n.rect.x + n.rect.w) as f64,
                y1: (n.rect.y + n.rect.h) as f64,
            });
            node.set_children(n.children.iter().map(|c| NodeId(*c)).collect::<Vec<_>>());
            if n.disabled {
                node.set_disabled();
            } else {
                if n.focusable {
                    node.add_action(Action::Focus);
                }
                if matches!(n.a11y.role, Role::Button | Role::CheckBox | Role::Switch | Role::RadioButton | Role::TabList | Role::ComboBox)
                {
                    node.add_action(Action::Click);
                }
            }
            (NodeId(n.id), node)
        })
        .collect();
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(NodeId(tree.root))),
        tree_id: TreeId::ROOT,
        focus: NodeId(tree.focus.unwrap_or(tree.root)),
    }
}

/// Placeholder tree used before the first frame has been laid out.
pub fn placeholder(title: &str) -> TreeUpdate {
    let mut root = Node::new(AkRole::Window);
    root.set_label(title);
    TreeUpdate { nodes: vec![(NodeId(1), root)], tree: Some(TreeInfo::new(NodeId(1))), tree_id: TreeId::ROOT, focus: NodeId(1) }
}

/// Maps an AccessKit action request to a toolkit action (`None` for unsupported actions).
pub fn action(req: &accesskit::ActionRequest) -> Option<A11yAction> {
    match req.action {
        Action::Focus => Some(A11yAction::Focus(req.target_node.0)),
        Action::Click => Some(A11yAction::Activate(req.target_node.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hey_core::Rect as HRect;
    use hey_widgets::{A11y, A11yNode};

    #[test]
    fn update_maps_roles_and_actions() {
        let mk = |id, a11y: A11y, children| A11yNode {
            id,
            rect: HRect::new(0.0, 0.0, 10.0, 10.0),
            a11y,
            disabled: false,
            focusable: true,
            children,
        };
        let tree = A11yTree {
            root: 1,
            nodes: vec![mk(1, A11y::new(Role::Generic, ""), vec![2]), mk(2, A11y::new(Role::Button, "OK"), vec![])],
            focus: Some(2),
        };
        let u = to_update(&tree, "App");
        assert_eq!(u.focus, NodeId(2));
        assert_eq!(u.nodes[0].1.role(), AkRole::Window);
        let (_, b) = &u.nodes[1];
        assert_eq!(b.role(), AkRole::Button);
        assert_eq!(b.label(), Some("OK"));
        assert!(b.supports_action(Action::Click) && b.supports_action(Action::Focus));
        let req = accesskit::ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: NodeId(2), data: None };
        assert_eq!(action(&req), Some(A11yAction::Activate(2)));
    }
}
