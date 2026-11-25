// Copyright (c) Vitaly Golik (mjojo)

//! MVU glue (ADR-4): widgets produce app messages via `Callback`, `MsgSink` accumulates them.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// Queue of app messages; cheap to clone (shared queue).
pub struct MsgSink<Msg> {
    queue: Rc<RefCell<VecDeque<Msg>>>,
}

impl<Msg> Clone for MsgSink<Msg> {
    fn clone(&self) -> Self {
        Self { queue: Rc::clone(&self.queue) }
    }
}

impl<Msg> Default for MsgSink<Msg> {
    fn default() -> Self {
        Self { queue: Rc::new(RefCell::new(VecDeque::new())) }
    }
}

impl<Msg> MsgSink<Msg> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn send(&self, m: Msg) {
        self.queue.borrow_mut().push_back(m);
    }
    /// Takes all accumulated messages in send order.
    pub fn drain(&self) -> Vec<Msg> {
        self.queue.borrow_mut().drain(..).collect()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.borrow().is_empty()
    }
}

/// Converts a widget event argument (`A`) into an app message.
pub struct Callback<Msg, A = ()> {
    f: Rc<dyn Fn(A) -> Msg>,
}

impl<Msg, A> Clone for Callback<Msg, A> {
    fn clone(&self) -> Self {
        Self { f: Rc::clone(&self.f) }
    }
}

impl<Msg, A> Callback<Msg, A> {
    pub fn new(f: impl Fn(A) -> Msg + 'static) -> Self {
        Self { f: Rc::new(f) }
    }
    pub fn call(&self, arg: A) -> Msg {
        (self.f)(arg)
    }
    pub fn emit(&self, sink: &MsgSink<Msg>, arg: A) {
        sink.send(self.call(arg));
    }
}

impl<Msg: Clone + 'static> Callback<Msg, ()> {
    /// Callback without arguments that always yields the same message.
    pub fn constant(m: Msg) -> Self {
        Self::new(move |()| m.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Inc,
        Set(i32),
    }

    #[test]
    fn sink_keeps_order_and_drains() {
        let s = MsgSink::new();
        let s2 = s.clone();
        s.send(Msg::Inc);
        s2.send(Msg::Set(3));
        assert_eq!(s.drain(), vec![Msg::Inc, Msg::Set(3)]);
        assert!(s2.is_empty());
    }

    #[test]
    fn callbacks_emit_into_sink() {
        let s = MsgSink::new();
        Callback::constant(Msg::Inc).emit(&s, ());
        Callback::new(Msg::Set).emit(&s, 7);
        assert_eq!(s.drain(), vec![Msg::Inc, Msg::Set(7)]);
    }
}
