// Copyright (c) Vitaly Golik (mjojo)

//! Fuzzes `Arena`: it must match a model and never resurrect stale ids.

#![no_main]

use hey_core::{Arena, NodeId};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut arena: Arena<u32> = Arena::new();
    let mut issued: Vec<(NodeId, Option<u32>)> = Vec::new();
    for pair in data.chunks(2) {
        let sel = pair.get(1).copied().unwrap_or(0) as usize;
        match pair[0] % 3 {
            0 => {
                let v = sel as u32;
                issued.push((arena.insert(v), Some(v)));
            }
            1 if !issued.is_empty() => {
                let slot = sel % issued.len();
                let (id, expect) = issued[slot];
                assert_eq!(arena.remove(id), expect);
                issued[slot].1 = None;
            }
            2 if !issued.is_empty() => {
                let (id, expect) = issued[sel % issued.len()];
                assert_eq!(arena.get(id).copied(), expect);
                assert_eq!(arena.contains(id), expect.is_some());
            }
            _ => {}
        }
        assert_eq!(arena.len(), issued.iter().filter(|(_, v)| v.is_some()).count());
        assert_eq!(arena.iter().count(), arena.len());
    }
});
