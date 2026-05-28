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
    steps: Box<[Rc<RefCell<BuildStep>>]>, // Rc<RefCell<>> is an antipattern (not necessarily wrong, but should be carefully considered. Usually / often, it means your data could be organised better)
}

impl BuildTask {
    pub fn new(id: usize) -> Self {
        let mut temp = Vec::new();
        for i in STeX::all() {
            let buildstep = BuildStep::new(i);
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
        // should be harmless, but note that you're borrowing all dependencies here, so if *any* is borrowed mutably at any point, your program will just crash.
        self.steps.iter().all(|k| k.borrow().dep.is_empty())
    }

    // Ok, this works, *but*: it returns a cycle *if and only if* self *itself* is part of the cycle (hence, the name makes perfect sense).
    // Possibly dangerous...? If self *depends on* a cycle... won't just nxt grow infinitely big...?
    // => But: can therefore be "optimized" - if you break when nxt.contains(inseter), you break if you have found *any* cycle.
    // If you do: checking whether dep.buildtaskid == self.id still tells you whether self is cyclic, but you can also store
    // the found cycle somewhere, so you never have to call is_cyclic for any of the tasks in the found cycle.
    // You can *also* mark the ones where you did *not* find a cycle and thus never look at the same task twice.
    pub fn is_cyclic(&self) -> Option<Vec<(BuildTaskId, STeX)>> {
        // consists of: a) Dependency (BuildTask+BuildStep) and b) the tasks it is a dependency *of*
        let mut f1 = self
            .steps
            .iter()
            .flat_map(|bstp| {
                let bor = bstp.borrow();
                bor.dep
                    .iter()
                    .map(|d| (d.clone(), vec![(self.id, bor.get_task_id())])) // vec, collect, another collect, and cloned can definitely be optimized, but probably needs a proper loop rather than map/flat_map etc because of borrow checker
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut visited = HashSet::new();
        while let Some((dep, ancestors)) = f1.pop() {
            // self is in cycle
            if dep.buildtaskid == self.id {
                return Some(ancestors);
            }
            let inseter = (dep.buildtaskid, dep.step.borrow().get_task_id());
            if !visited.contains(&inseter) {
                visited.insert(inseter);
                let y = dep.step.borrow().dep.clone();
                for mk in y {
                    // if ancestors contains mk, you found a cycle (not including self)!
                    let mut nxt = ancestors.clone();
                    nxt.push((dep.buildtaskid, dep.step.borrow().get_task_id())); // <- this is just inseter again
                    f1.push((mk, nxt));
                }
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
