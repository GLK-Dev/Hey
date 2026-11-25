// Copyright (c) Vitaly Golik (mjojo)

//! Property test: `Arena` must behave like a model map and never resurrect stale (generation-mismatched) ids.

use hey_core::{Arena, NodeId};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    Insert(u32),
    Remove(usize),
    Get(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![any::<u32>().prop_map(Op::Insert), (0usize..32).prop_map(Op::Remove), (0usize..32).prop_map(Op::Get)]
}

proptest! {
    #[test]
    fn matches_model_and_rejects_stale_ids(ops in prop::collection::vec(op(), 0..200)) {
        let mut arena: Arena<u32> = Arena::new();
        // Every id ever issued, with the value it currently maps to (None once removed).
        let mut issued: Vec<(NodeId, Option<u32>)> = Vec::new();
        for o in ops {
            match o {
                Op::Insert(v) => issued.push((arena.insert(v), Some(v))),
                Op::Remove(i) if !issued.is_empty() => {
                    let slot = i % issued.len();
                    let (id, expect) = issued[slot];
                    prop_assert_eq!(arena.remove(id), expect);
                    issued[slot].1 = None;
                }
                Op::Get(i) if !issued.is_empty() => {
                    let (id, expect) = issued[i % issued.len()];
                    prop_assert_eq!(arena.get(id).copied(), expect);
                    prop_assert_eq!(arena.contains(id), expect.is_some());
                }
                _ => {}
            }
            prop_assert_eq!(arena.len(), issued.iter().filter(|(_, v)| v.is_some()).count());
            prop_assert_eq!(arena.iter().count(), arena.len());
        }
    }
}
