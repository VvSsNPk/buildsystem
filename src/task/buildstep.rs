use rand::random_bool;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, OnceLock},
};

use crate::task::buildtask::BuildTaskId;

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct BuildStep {
    task: STeX,          // <- aren't those targets?
    target: BuildTarget, // <- ????
    taskstate: TaskState,
    should_fail: bool, // <- ???? (for tests only!)
    pub dep: Vec<Dependency>,
}

impl BuildStep {
    pub fn new(stex: STeX) -> Self {
        Self {
            task: stex,
            target: BuildTarget {},
            taskstate: TaskState::None,
            should_fail: random_bool(0.70),
            dep: Vec::new(),
        }
    }
    #[inline]
    pub fn get_task_id(&self) -> STeX {
        self.task
    }

    pub fn add_dependency(&mut self, task_id: BuildTaskId, dep: Rc<RefCell<BuildStep>>) {
        let dp = Dependency::new(task_id, dep);
        if !self.dep.contains(&dp) {
            self.dep.push(dp);
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct BuildTarget {}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub enum TaskState {
    None,
    Running,
    Done,
    Queued,
    Finished,
    Failed,
}

// The STeX type represents *something* but it's not obvious what (build targets?)
// Generally, for readability alone it may be a good idea do have a trait for that thing.
// There will probably be certain constraints those things will have.
//
// Note also, that having an enum for your build targets implies that they're hardcoded and known statically at compile time,
// which is probably not going to remain true in the future.
#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord, Copy)]
pub enum STeX {
    Pdflatex1,
    Bibtex,
    Pdflatex2,
    Check,
}

impl STeX {
    pub const fn all() -> [Self; 4] {
        [Self::Pdflatex1, Self::Bibtex, Self::Pdflatex2, Self::Check]
    }
    // basically unnecessary - Self is Copy, array of 4 is more efficient, actually (and less "duplication"!)
    /*pub fn initialize() -> &'static [STeX] {
        STEX.get_or_init(|| [STeX::Pdflatex1, STeX::Bibtex, STeX::Pdflatex2, STeX::Check])
    }*/
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Clone)]
pub struct Dependency {
    pub buildtaskid: BuildTaskId,
    // This assumes that you can, and have already, resolved all possible dependencies to the corresponding build step.
    // Note that the get_dependencies() method can't know the build steps *themselves*.
    pub step: Rc<RefCell<BuildStep>>, // antipattern
                                      // Do we expect the build step *itself* to change? Probably not, only the *dependencies*-field (and the state), no?
                                      // Possibly: Move the RefCell to the dependencies field => borrowing only needed if/when you acces those.
                                      // The *state* is probably going to be a *finite* type?
                                      // => std::cell::Cell, works with Copy types, not borrowing needed at all.
                                      //
                                      // In async context: Probably *AtomicU8*. Can be updated asynchronously. Implement TaskState:From<AtomicU8>, or potentially better,
                                      // do: struct TaskState(AtomicU8), move the update/enum-conversion into that type => isolates the annoying complexity so you don't have to deal with it later.
}

impl Dependency {
    pub fn new(buildtaskid: BuildTaskId, step: Rc<RefCell<BuildStep>>) -> Self {
        Self { buildtaskid, step }
    }
}
