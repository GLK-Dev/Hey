// Copyright (c) Vitaly Golik (mjojo)

//! Generational arena (ADR-1, ADR-2): a stale `NodeId` never lands in someone else's slot.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    pub index: u32,
    pub generation: u32,
}

enum Slot<T> {
    Occupied { generation: u32, value: T },
    Free { generation: u32, next_free: Option<u32> },
}

pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free_head: Option<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self { slots: Vec::new(), free_head: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, value: T) -> NodeId {
        self.len += 1;
        if let Some(index) = self.free_head {
            let Slot::Free { generation, next_free } = self.slots[index as usize] else {
                unreachable!("free list points at an occupied slot");
            };
            self.free_head = next_free;
            self.slots[index as usize] = Slot::Occupied { generation, value };
            NodeId { index, generation }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot::Occupied { generation: 0, value });
            NodeId { index, generation: 0 }
        }
    }

    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        match self.slots.get(id.index as usize)? {
            Slot::Occupied { generation, .. } if *generation == id.generation => {}
            _ => return None,
        }
        let new_gen = id.generation.wrapping_add(1);
        let old = std::mem::replace(&mut self.slots[id.index as usize], Slot::Free { generation: new_gen, next_free: self.free_head });
        self.free_head = Some(id.index);
        self.len -= 1;
        match old {
            Slot::Occupied { value, .. } => Some(value),
            Slot::Free { .. } => unreachable!(),
        }
    }

    pub fn get(&self, id: NodeId) -> Option<&T> {
        match self.slots.get(id.index as usize)? {
            Slot::Occupied { generation, value } if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        match self.slots.get_mut(id.index as usize)? {
            Slot::Occupied { generation, value } if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| match s {
            Slot::Occupied { generation, value } => Some((NodeId { index: i as u32, generation: *generation }, value)),
            Slot::Free { .. } => None,
        })
    }
}

impl<T> std::ops::Index<NodeId> for Arena<T> {
    type Output = T;
    fn index(&self, id: NodeId) -> &T {
        self.get(id).expect("stale or invalid NodeId")
    }
}

impl<T> std::ops::IndexMut<NodeId> for Arena<T> {
    fn index_mut(&mut self, id: NodeId) -> &mut T {
        self.get_mut(id).expect("stale or invalid NodeId")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut a = Arena::new();
        let x = a.insert("x");
        let y = a.insert("y");
        assert_eq!((a[x], a[y], a.len()), ("x", "y", 2));
        assert_eq!(a.remove(x), Some("x"));
        assert_eq!(a.len(), 1);
        assert!(a.get(x).is_none());
    }

    #[test]
    fn slot_reuse_invalidates_old_id() {
        let mut a = Arena::new();
        let old = a.insert(1);
        a.remove(old);
        let new = a.insert(2);
        assert_eq!(new.index, old.index);
        assert_ne!(new.generation, old.generation);
        assert!(a.get(old).is_none());
        assert_eq!(a[new], 2);
        assert_eq!(a.remove(old), None, "double removal by a stale id is forbidden");
    }

    #[test]
    fn free_list_is_lifo_and_iter_skips_free() {
        let mut a = Arena::new();
        let ids: Vec<_> = (0..4).map(|i| a.insert(i)).collect();
        a.remove(ids[1]);
        a.remove(ids[2]);
        assert_eq!(a.insert(9).index, ids[2].index);
        let vals: Vec<_> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(vals, vec![0, 9, 3]);
    }
}
