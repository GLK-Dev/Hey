// Copyright (c) Vitaly Golik (mjojo)

//! Tree on top of `Arena`: parent/children, subtree removal, depth-first traversal.

use crate::arena::{Arena, NodeId};

struct Entry<T> {
    value: T,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

pub struct Tree<T> {
    arena: Arena<Entry<T>>,
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Tree<T> {
    pub fn new() -> Self {
        Self { arena: Arena::new() }
    }

    pub fn len(&self) -> usize {
        self.arena.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Adds a node as the last child of `parent` (or as the root if `None`).
    /// If `parent` is invalid, returns `None` and inserts nothing.
    pub fn add(&mut self, value: T, parent: Option<NodeId>) -> Option<NodeId> {
        if let Some(p) = parent {
            if !self.arena.contains(p) {
                return None;
            }
        }
        let id = self.arena.insert(Entry { value, parent, children: Vec::new() });
        if let Some(p) = parent {
            self.arena[p].children.push(id);
        }
        Some(id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.arena.contains(id)
    }

    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.arena.get(id).map(|e| &e.value)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.arena.get_mut(id).map(|e| &mut e.value)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.arena.get(id)?.parent
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.arena.get(id).map_or(&[], |e| e.children.as_slice())
    }

    /// Sets the children order. `order` must be a permutation of the current children; otherwise `false`.
    pub fn reorder_children(&mut self, parent: NodeId, order: &[NodeId]) -> bool {
        let Some(e) = self.arena.get_mut(parent) else { return false };
        let a: std::collections::HashSet<NodeId> = e.children.iter().copied().collect();
        let b: std::collections::HashSet<NodeId> = order.iter().copied().collect();
        if a != b || order.len() != e.children.len() {
            return false;
        }
        e.children = order.to_vec();
        true
    }

    /// Removes a node together with its subtree; returns the number of removed nodes.
    pub fn remove_subtree(&mut self, id: NodeId) -> usize {
        let Some(entry) = self.arena.get(id) else { return 0 };
        if let Some(p) = entry.parent {
            if let Some(pe) = self.arena.get_mut(p) {
                pe.children.retain(|c| *c != id);
            }
        }
        let mut stack = vec![id];
        let mut removed = 0;
        while let Some(n) = stack.pop() {
            if let Some(e) = self.arena.remove(n) {
                stack.extend(e.children);
                removed += 1;
            }
        }
        removed
    }

    /// Depth-first, pre-order traversal (parent before children, children left to right).
    pub fn dfs(&self, root: NodeId) -> Dfs<'_, T> {
        let stack = if self.contains(root) { vec![root] } else { Vec::new() };
        Dfs { tree: self, stack }
    }
}

pub struct Dfs<'a, T> {
    tree: &'a Tree<T>,
    stack: Vec<NodeId>,
}

impl<'a, T> Iterator for Dfs<'a, T> {
    type Item = (NodeId, &'a T);
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let e = self.tree.arena.get(id)?;
        self.stack.extend(e.children.iter().rev());
        Some((id, &e.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Tree<&'static str>, NodeId, NodeId, NodeId, NodeId) {
        let mut t = Tree::new();
        let r = t.add("r", None).unwrap();
        let a = t.add("a", Some(r)).unwrap();
        let b = t.add("b", Some(r)).unwrap();
        let a1 = t.add("a1", Some(a)).unwrap();
        (t, r, a, b, a1)
    }

    #[test]
    fn dfs_preorder() {
        let (t, r, ..) = sample();
        let v: Vec<_> = t.dfs(r).map(|(_, s)| *s).collect();
        assert_eq!(v, ["r", "a", "a1", "b"]);
    }

    #[test]
    fn remove_subtree_detaches_and_invalidates() {
        let (mut t, r, a, b, a1) = sample();
        assert_eq!(t.remove_subtree(a), 2);
        assert!(!t.contains(a) && !t.contains(a1));
        assert_eq!(t.children(r), &[b]);
        assert_eq!(t.len(), 2);
        assert_eq!(t.remove_subtree(a), 0);
    }

    #[test]
    fn add_to_stale_parent_fails() {
        let (mut t, _, a, ..) = sample();
        t.remove_subtree(a);
        assert!(t.add("x", Some(a)).is_none());
    }
}
