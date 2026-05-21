//! Bündelt Subagent-Timeline-Events und wendet sie nach 50 ms Ruhephase an.

use crate::agent_wire::AgentEvent;
use gloo_timers::future::TimeoutFuture;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct SubagentEventDebounce {
    pending: Rc<RefCell<Vec<AgentEvent>>>,
    generation: Rc<RefCell<u64>>,
}

impl SubagentEventDebounce {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Rc::new(RefCell::new(Vec::new())),
            generation: Rc::new(RefCell::new(0)),
        }
    }

    pub fn push(&self, ev: AgentEvent, flush: Rc<dyn Fn(Vec<AgentEvent>)>) {
        self.pending.borrow_mut().push(ev);
        *self.generation.borrow_mut() += 1;
        let tick = *self.generation.borrow();
        let pending = self.pending.clone();
        let generation = self.generation.clone();
        leptos::task::spawn_local(async move {
            TimeoutFuture::new(50).await;
            if *generation.borrow() != tick {
                return;
            }
            let batch: Vec<AgentEvent> = pending.borrow_mut().drain(..).collect();
            if !batch.is_empty() {
                flush(batch);
            }
        });
    }
}

#[must_use]
pub fn is_subagent_timeline_event(ev: &AgentEvent) -> bool {
    matches!(
        ev,
        AgentEvent::SubagentStarted { .. }
            | AgentEvent::SubagentStep { .. }
            | AgentEvent::SubagentToolCall { .. }
            | AgentEvent::SubagentFinished { .. }
    )
}
