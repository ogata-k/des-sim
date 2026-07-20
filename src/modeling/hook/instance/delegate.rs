//! The `delegate` module provides the `HookDelegate` struct, an internal component
//! responsible for managing and dispatching simulation lifecycle events to multiple
//! registered `Hook` implementations.
//!
//! It acts as a central hub for hooks, ensuring that "before" hooks are called
//! in registration order and "after" hooks in reverse order, maintaining predictable
//! execution flow.

use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::hook::instance::SharedHook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};

/// An internal orchestrator that manages and broadcasts lifecycle events to multiple registered `Hook`s.
///
/// `HookDelegate` implements the `Hook` trait itself, allowing it to act as a single entry point
/// for the simulation engine. It handles the orderly propagation of lifecycle events by maintaining
/// the registration order for "before" hooks and reversing it for "after" hooks, ensuring
/// predictable side effect execution.
pub(crate) struct HookDelegate<E, M: Model<E>> {
    hooks: Vec<Box<dyn Hook<E, M>>>,
}

impl<E, M: Model<E>> Default for HookDelegate<E, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, M: Model<E>> Hook<E, M> for HookDelegate<E, M> {
    // --- Simulation Lifecycle ---

    fn before_simulation(&self, model: &M) {
        self.delegate(|hook| hook.before_simulation(model))
    }

    fn after_simulation(&self, model: &M, end_tick: SimTime) {
        self.reverse_delegate(|hook| hook.after_simulation(model, end_tick))
    }

    // --- Tick Lifecycle ---

    fn before_tick(&self, model: &M, current_tick: SimTime, skipped_duration: Duration) {
        self.delegate(|hook| hook.before_tick(model, current_tick, skipped_duration))
    }

    fn after_tick(&self, model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
        self.reverse_delegate(|hook| hook.after_tick(model, current_tick, last_micro_step))
    }

    fn before_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.delegate(|hook| hook.before_micro_step(model, current_tick, current_micro_step))
    }

    fn after_micro_step(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.reverse_delegate(|hook| hook.after_micro_step(model, current_tick, current_micro_step))
    }

    fn on_discard_remain_micro_step(
        &self,
        model: &M,
        current_tick: SimTime,
        first_discarded_micro_step: MicroStep,
        discarded_sources: &[SourceReadyEntry],
        discarded_events: &[Event<E>],
    ) {
        self.reverse_delegate(|hook| {
            hook.on_discard_remain_micro_step(
                model,
                current_tick,
                first_discarded_micro_step,
                discarded_sources,
                discarded_events,
            )
        })
    }

    // --- Source Lifecycle ---

    fn before_register_source(&self, model: &M, name: &str) {
        self.delegate(|hook| hook.before_register_source(model, name))
    }

    fn after_register_source(&self, model: &M, name: &str) {
        self.reverse_delegate(|hook| hook.after_register_source(model, name))
    }

    fn before_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.delegate(|hook| hook.before_source_phase(model, current_tick, current_micro_step))
    }

    fn before_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.delegate(|hook| {
            hook.before_source(model, current_tick, current_micro_step, source_view)
        })
    }

    fn after_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
        computed_next_fire: Option<SimTime>,
    ) {
        self.reverse_delegate(|hook| {
            hook.after_source(
                model,
                current_tick,
                current_micro_step,
                source_view,
                computed_next_fire,
            )
        })
    }

    fn cancel_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        source_view: &SourceView,
    ) {
        self.delegate(|hook| {
            hook.cancel_source(
                model,
                current_tick,
                current_micro_step,
                scheduled_at,
                source_view,
            )
        })
    }

    fn discard_source(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        source_view: &SourceView,
    ) {
        self.delegate(|hook| {
            hook.discard_source(model, current_tick, current_micro_step, source_view)
        })
    }

    fn after_source_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.reverse_delegate(|hook| {
            hook.after_source_phase(model, current_tick, current_micro_step)
        })
    }

    // --- Event Lifecycle ---

    fn before_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.delegate(|hook| hook.before_event_phase(model, current_tick, current_micro_step))
    }

    fn before_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        self.delegate(|hook| hook.before_event(model, current_tick, current_micro_step, event))
    }

    fn after_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        self.reverse_delegate(|hook| {
            hook.after_event(model, current_tick, current_micro_step, event)
        })
    }

    fn cancel_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        scheduled_at: SimTime,
        event: &Event<E>,
    ) {
        self.delegate(|hook| {
            hook.cancel_event(model, current_tick, current_micro_step, scheduled_at, event)
        })
    }

    fn discard_event(
        &self,
        model: &M,
        current_tick: SimTime,
        current_micro_step: MicroStep,
        event: &Event<E>,
    ) {
        self.delegate(|hook| hook.discard_event(model, current_tick, current_micro_step, event))
    }

    fn after_event_phase(&self, model: &M, current_tick: SimTime, current_micro_step: MicroStep) {
        self.reverse_delegate(|hook| {
            hook.after_event_phase(model, current_tick, current_micro_step)
        })
    }
}

impl<E, M: Model<E>> HookDelegate<E, M> {
    /// Create `HookDelegate` instance.
    pub(crate) fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Adds a new `Hook` to the delegate collection.
    pub(crate) fn add_hook<H>(&mut self, hook: H)
    where
        H: Hook<E, M> + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    /// Adds a shared `Hook` to the delegate collection.
    pub(crate) fn add_shared_hook<H>(&mut self, shared_hook: SharedHook<E, M, H>)
    where
        E: 'static,
        M: 'static,
        H: Hook<E, M> + 'static,
    {
        self.hooks.push(Box::new(shared_hook));
    }

    /// Helper to iterate through all hooks.
    fn delegate<F, R>(&self, f: F)
    where
        F: Fn(&dyn Hook<E, M>) -> R,
    {
        for hook in self.hooks.iter() {
            f(hook.as_ref());
        }
    }

    /// Helper to iterate through all hooks in reverse order.
    fn reverse_delegate<F, R>(&self, f: F)
    where
        F: Fn(&dyn Hook<E, M>) -> R,
    {
        for hook in self.hooks.iter().rev() {
            f(hook.as_ref());
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.hooks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::EventContext;
    use crate::modeling::event::EventPriority;
    use crate::primitive::id::{EventId, SourceId};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    struct MockModel;
    impl Model<()> for MockModel {
        fn handle_event(&mut self, _context: &mut EventContext<(), Self>, _event: &Event<()>) {
            // No-op
        }
    }

    struct MockHook {
        call_order: Rc<RefCell<Vec<usize>>>,
        id: usize,
    }

    impl MockHook {
        fn new(call_order: Rc<RefCell<Vec<usize>>>, id: usize) -> Self {
            Self { call_order, id }
        }
    }

    impl Hook<(), MockModel> for MockHook {
        fn before_simulation(&self, _model: &MockModel) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_simulation(&self, _model: &MockModel, _end_tick: SimTime) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_tick(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _skipped_duration: Duration,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_tick(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _last_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_micro_step(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_micro_step(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn on_discard_remain_micro_step(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _first_discarded_micro_step: MicroStep,
            _discarded_sources: &[SourceReadyEntry],
            _discarded_events: &[Event<()>],
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_register_source(&self, _model: &MockModel, _name: &str) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_register_source(&self, _model: &MockModel, _name: &str) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_source_phase(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_source(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_source(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
            _computed_next_fire: Option<SimTime>,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn cancel_source(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _source_view: &SourceView,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn discard_source(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _source_view: &SourceView,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_source_phase(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_event_phase(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn before_event(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<()>,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_event(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<()>,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn cancel_event(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _scheduled_at: SimTime,
            _event: &Event<()>,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn discard_event(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
            _event: &Event<()>,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }

        fn after_event_phase(
            &self,
            _model: &MockModel,
            _current_tick: SimTime,
            _current_micro_step: MicroStep,
        ) {
            self.call_order.borrow_mut().push(self.id);
        }
    }

    #[test]
    fn test_default_implementation() {
        let delegate: HookDelegate<(), MockModel> = HookDelegate::default();
        assert!(delegate.hooks.is_empty());
    }

    #[test]
    fn test_add_shared_hook() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        let shared_hook = SharedHook::new(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_shared_hook(shared_hook);

        let model = MockModel;
        delegate.before_simulation(&model);

        assert_eq!(*call_order.borrow(), vec![1]);
    }

    #[test]
    fn test_before_simulation_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.before_simulation(&model);

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_simulation_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.after_simulation(&model, SimTime::from_ticks(10));

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_before_tick_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.before_tick(&model, SimTime::from_ticks(0), Duration::zero());

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_tick_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.after_tick(&model, SimTime::from_ticks(10), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_before_micro_step_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.before_micro_step(&model, SimTime::from_ticks(0), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_micro_step_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.after_micro_step(&model, SimTime::from_ticks(0), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_on_discard_remain_micro_step_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.on_discard_remain_micro_step(
            &model,
            SimTime::from_ticks(0),
            MicroStep::zero(),
            &[],
            &[],
        );

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_before_register_source_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.before_register_source(&model, "test_source");

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_register_source_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.after_register_source(&model, "test_source");

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_before_source_phase_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.before_source_phase(&model, SimTime::from_ticks(0), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_before_source_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let source_view = SourceView::new(SourceId::new(0), Arc::from("test_source"));
        delegate.before_source(
            &model,
            SimTime::from_ticks(0),
            MicroStep::zero(),
            &source_view,
        );

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_source_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let source_view = SourceView::new(SourceId::new(0), Arc::from("test_source"));
        delegate.after_source(
            &model,
            SimTime::from_ticks(0),
            MicroStep::zero(),
            &source_view,
            None,
        );

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_cancel_source_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let source_view = SourceView::new(SourceId::new(0), Arc::from("test_source"));
        delegate.cancel_source(
            &model,
            SimTime::from_ticks(0),
            MicroStep::zero(),
            SimTime::from_ticks(10),
            &source_view,
        );

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_discard_source_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let source_view = SourceView::new(SourceId::new(0), Arc::from("test_source"));
        delegate.discard_source(
            &model,
            SimTime::from_ticks(0),
            MicroStep::zero(),
            &source_view,
        );

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_source_phase_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.after_source_phase(&model, SimTime::from_ticks(0), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_before_event_phase_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.before_event_phase(&model, SimTime::from_ticks(0), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_before_event_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let event = Event::new(EventId::new(0), EventPriority::minimum(), ());
        delegate.before_event(&model, SimTime::from_ticks(0), MicroStep::zero(), &event);

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_event_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let event = Event::new(EventId::new(0), EventPriority::minimum(), ());
        delegate.after_event(&model, SimTime::from_ticks(0), MicroStep::zero(), &event);

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn test_cancel_event_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let event = Event::new(EventId::new(0), EventPriority::minimum(), ());
        delegate.cancel_event(
            &model,
            SimTime::from_ticks(0),
            MicroStep::zero(),
            SimTime::from_ticks(10),
            &event,
        );

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_discard_event_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        let event = Event::new(EventId::new(0), EventPriority::minimum(), ());
        delegate.discard_event(&model, SimTime::from_ticks(0), MicroStep::zero(), &event);

        assert_eq!(*call_order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn test_after_event_phase_reverse_delegate_order() {
        let mut delegate = HookDelegate::new();
        let call_order = Rc::new(RefCell::new(Vec::new()));

        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 1));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 2));
        delegate.add_hook(MockHook::new(Rc::clone(&call_order), 3));

        let model = MockModel;
        delegate.after_event_phase(&model, SimTime::from_ticks(0), MicroStep::zero());

        assert_eq!(*call_order.borrow(), vec![3, 2, 1]);
    }
}
