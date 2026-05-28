use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::task::buildstep::{BuildStep, STeX};

#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Eq, Ord, Copy)]
pub struct BuildTaskId(usize);

impl From<usize> for BuildTaskId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<BuildTaskId> for usize {
    fn from(value: BuildTaskId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub struct BuildTask {
    id: BuildTaskId,
    steps: Box<[Rc<RefCell<BuildStep>>]>,
}

impl BuildTask {
    pub fn new(id: usize) -> Self {
        let mut temp = Vec::new();
        for i in STeX::initialize() {
            let buildstep = BuildStep::new(*i);
            temp.push(Rc::new(RefCell::new(buildstep)));
        }
        Self {
            id: BuildTaskId(id),
            steps: temp.into(),
        }
    }
    pub fn get_id(&self) -> BuildTaskId {
        self.id
    }

    pub fn get_steps(&self) -> &[Rc<RefCell<BuildStep>>] {
        &self.steps
    }

    pub fn root_task(&self) -> bool {
        self.steps.iter().all(|k| k.borrow().dep.is_empty())
    }

    pub fn is_cyclic(&self) -> Option<Vec<(BuildTaskId, STeX)>> {
        let mut f1 = self
            .steps
            .iter()
            .flat_map(|bstp| {
                let temp = bstp
                    .borrow()
                    .dep
                    .iter()
                    .cloned()
                    .map(|d| (d, vec![(self.id, bstp.borrow().get_task_id())]))
                    .collect::<Vec<_>>();
                temp
            })
            .collect::<Vec<_>>();
        let mut visited = HashSet::new();
        while let Some(x) = f1.pop() {
            let inseter = (x.0.buildtaskid, x.0.step.borrow().get_task_id());
            if !visited.contains(&inseter) {
                visited.insert(inseter);
                let y = x.0.step.borrow().dep.clone();
                for mk in y {
                    let mut nxt = x.1.clone();
                    nxt.push((x.0.buildtaskid, x.0.step.borrow().get_task_id()));
                    f1.push((mk, nxt));
                }
            }
            if inseter.0 == self.id {
                return Some(x.1);
            }
        }
        None
    }

    pub fn add_dep_to_step(
        &mut self,
        stex: STeX,
        task_id: BuildTaskId,
        dep: Rc<RefCell<BuildStep>>,
    ) {
        let mut temp = self.get_step(stex);
        temp.borrow_mut().add_dependency(task_id, dep);
    }

    pub fn get_step(&self, step: STeX) -> Rc<RefCell<BuildStep>> {
        let x = self
            .steps
            .iter()
            .find(|step1| step1.borrow().get_task_id() == step)
            .unwrap();
        x.clone()
    }
}
