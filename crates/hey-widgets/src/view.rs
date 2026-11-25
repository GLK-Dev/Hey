// Copyright (c) Vitaly Golik (mjojo)

//! Declarative tree description and reconciliation (reconcile) with the live `Ui` by `Key`.

use crate::ui::Ui;
use hey_core::{Key, NodeId};
use hey_layout::Style;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

type Build<Msg> = Box<dyn FnOnce(&mut Ui<Msg>, NodeId) -> NodeId>;

enum Kind<Msg> {
    Leaf { hash: u64, build: Build<Msg> },
    Container { style: Style, children: Vec<View<Msg>> },
}

/// Description of a single node: key + either a widget (recreated when the props hash changes),
/// or a container with children (always kept, style is updated).
pub struct View<Msg> {
    key: Key,
    kind: Kind<Msg>,
}

/// Container marker in `Ui::keys` (a leaf hash is always odd).
const CONTAINER: u64 = 0;

impl<Msg: 'static> View<Msg> {
    /// Leaf: `props` is everything the look/behavior depends on (text, value…);
    /// `build` creates the widget in the given parent and returns its `NodeId`.
    pub fn leaf(key: impl Into<Key>, props: impl Hash, build: impl FnOnce(&mut Ui<Msg>, NodeId) -> NodeId + 'static) -> Self {
        let mut h = DefaultHasher::new();
        props.hash(&mut h);
        View { key: key.into(), kind: Kind::Leaf { hash: h.finish() | 1, build: Box::new(build) } }
    }

    pub fn container(key: impl Into<Key>, style: Style, children: Vec<View<Msg>>) -> Self {
        View { key: key.into(), kind: Kind::Container { style, children } }
    }
}

impl<Msg: 'static> Ui<Msg> {
    /// Brings the children of `parent` in line with the `views` list: nodes with the same key and hash are kept
    /// (together with their state), changed ones are recreated with state carried over,
    /// extra ones are removed, order follows the list. Children without a key stay first.
    pub fn reconcile(&mut self, parent: NodeId, views: Vec<View<Msg>>) {
        let old: Vec<NodeId> = self.tree.children(parent).to_vec();
        let mut order: Vec<NodeId> = old.iter().copied().filter(|id| !self.keys.contains_key(id)).collect();
        let mut pool: HashMap<Key, NodeId> = HashMap::new();
        for id in &old {
            if let Some((k, _)) = self.keys.get(id) {
                if let Some(dup) = pool.insert(k.clone(), *id) {
                    self.remove_subtree(dup);
                }
            }
        }

        for v in views {
            let found = pool.remove(&v.key).map(|id| (id, self.keys[&id].1));
            let id = match v.kind {
                Kind::Leaf { hash, build } => match found {
                    Some((id, h)) if h == hash => id,
                    other => {
                        let (state, had_focus) = match other {
                            Some((old_id, _)) => {
                                let st = self.nodes.get(&old_id).and_then(|n| n.widget.save_state());
                                let f = self.focused() == Some(old_id);
                                self.remove_subtree(old_id);
                                (st, f)
                            }
                            None => (None, false),
                        };
                        let id = build(self, parent);
                        if let (Some(st), Some(n)) = (state, self.nodes.get_mut(&id)) {
                            n.widget.load_state(st);
                        }
                        if had_focus {
                            self.focus(id);
                        }
                        self.keys.insert(id, (v.key, hash));
                        id
                    }
                },
                Kind::Container { style, children } => {
                    let id = match found {
                        Some((id, CONTAINER)) => {
                            self.tree.set_style(id, style);
                            id
                        }
                        other => {
                            if let Some((old_id, _)) = other {
                                self.remove_subtree(old_id);
                            }
                            let id = self.add_container(parent, style);
                            self.keys.insert(id, (v.key, CONTAINER));
                            id
                        }
                    };
                    self.reconcile(id, children);
                    id
                }
            };
            order.push(id);
        }

        for (_, id) in pool {
            self.remove_subtree(id);
        }
        self.tree.reorder_children(parent, &order);
    }
}
