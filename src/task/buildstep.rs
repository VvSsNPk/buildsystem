use rand::random_bool;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, OnceLock},
};

use crate::task::buildtask::BuildTaskId;

static STEX: OnceLock<[STeX; 4]> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct BuildStep {
    task: STeX,
    target: BuildTarget,
    taskstate: TaskState,
    should_fail: bool,
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord, Copy)]
pub enum STeX {
    Pdflatex1,
    Bibtex,
    Pdflatex2,
    Check,
}

impl STeX {
    pub fn initialize() -> &'static [STeX] {
        STEX.get_or_init(|| [STeX::Pdflatex1, STeX::Bibtex, STeX::Pdflatex2, STeX::Check])
    }
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Clone)]
pub struct Dependency {
    pub buildtaskid: BuildTaskId,
    pub step: Rc<RefCell<BuildStep>>,
}

impl Dependency {
    pub fn new(buildtaskid: BuildTaskId, step: Rc<RefCell<BuildStep>>) -> Self {
        Self { buildtaskid, step }
    }
}
