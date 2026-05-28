use crate::task::{
    TaskMap,
    buildtask::{BuildTask, BuildTaskId},
};

pub struct RunnningQueue {
    pub done: Vec<BuildTask>,
    pub blocked: Vec<BuildTask>,
    pub queued: Vec<BuildTask>,
    pub running: Vec<BuildTask>,
    pub failed: Vec<BuildTask>,
}

pub struct FinishedQueue {
    pub done: Vec<BuildTask>,
    pub failed: Vec<BuildTask>,
}

pub enum QueueState {
    Running(RunnningQueue),
    Idle,
    Finished(FinishedQueue),
}

pub struct Queue {
    pub map: TaskMap<BuildTaskId>,
    pub queue: QueueState,
}

impl Queue {
    pub fn new(map: TaskMap<BuildTaskId>) -> Self {
        Self {
            map,
            queue: QueueState::Idle,
        }
    }

    pub fn start(&mut self) {
        let mut store = self.map.map.values().cloned().collect::<Vec<_>>(); // that seems... expensive?
        store.sort_by(|b1, b2| todo!()); // why not just sort them directly? (depends on what happens afterwards, of course. .sort_by likely won't be applicable (needs binary relation))
    }
}
