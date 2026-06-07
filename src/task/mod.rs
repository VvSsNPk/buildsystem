use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use petgraph::graph::{DiGraph, NodeIndex};
use rand::{
    RngExt, rng,
    seq::{IndexedRandom, IteratorRandom},
};

use crate::task::{
    buildstep::{BuildStep, Dependency, STeX},
    buildtask::{BuildTask, BuildTaskId},
};

pub mod buildstep;
pub mod buildtask;
pub mod queue;

#[derive(Default, Debug)]
pub struct TaskMap<K: Hash> {
    pub map: HashMap<K, BuildTask>,
}

impl<K: Hash> TaskMap<K> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl TaskMap<BuildTaskId> {
    pub fn entirely_random(max_tasks: usize, m0: usize, m: usize) -> Self {
        let mut taskmap = TaskMap::new();
        let mut rng = rng();
        for i in 0..max_tasks {
            taskmap.map.insert(BuildTaskId::from(i), BuildTask::new(i));
        }
        for i in 0..m0 {
            // TODO
            for j in i + 1..m0 {
                taskmap.create_link(i, j);
            }
        }

        for i in m0..max_tasks {
            let mut possible_targets: Vec<BuildTaskId> = (0..i).map(BuildTaskId::from).collect();
            for _ in 0..m {
                let j = possible_targets
                    .choose_weighted(&mut rng, |&y| {
                        taskmap
                            .get_build_step(y, STeX::Pdflatex2)
                            .borrow()
                            .dep
                            .len()
                    })
                    .unwrap()
                    .to_owned();
                let num: usize = j.into();
                taskmap.create_link(i, num);
                possible_targets.retain(|&x| x != j);
            }
        }
        taskmap
    }
    pub fn create_map(max_tasks: usize) -> (Self, HashSet<(usize, usize, usize)>) {
        let mut taskmap = TaskMap::new();
        for i in 0..max_tasks {
            let b_task = BuildTask::new(i);
            taskmap.map.insert(BuildTaskId::from(i), b_task);
        }

        let mut created = HashSet::new();
        let to_choose = max_tasks / 2;
        loop {
            let x = (0..max_tasks).sample(&mut rng(), 3);
            if !created.insert((x[0], x[1], x[2])) {
                continue;
            }
            //println!("({},{},{})", x[0], x[1], x[2]);
            taskmap.create_three_dep(
                BuildTaskId::from(x[0]),
                BuildTaskId::from(x[1]),
                BuildTaskId::from(x[2]),
            );
            if created.len() >= to_choose {
                break;
            }
        }
        (taskmap, created)
    }
    pub fn add_map(map: HashMap<BuildTaskId, BuildTask>) -> Self {
        Self { map }
    }

    pub fn get_build_step(&self, id: BuildTaskId, stex: STeX) -> Rc<RefCell<BuildStep>> {
        self.map.get(&id).unwrap().get_step(stex)
    }

    pub fn get_pdf_latex(&self, id: BuildTaskId) -> Rc<RefCell<BuildStep>> {
        self.get_build_step(id, STeX::Pdflatex2)
    }

    pub fn create_link(&mut self, from: usize, to: usize) {
        let b1 = self
            .map
            .entry(BuildTaskId::from(from))
            .or_insert_with(|| BuildTask::new(from))
            .get_step(STeX::Pdflatex2);
        let b2 = self
            .map
            .entry(BuildTaskId::from(to))
            .or_insert_with(|| BuildTask::new(to))
            .get_step(STeX::Pdflatex2);
        b2.borrow_mut().add_dependency(BuildTaskId::from(from), b1);
    }

    pub fn create_three_dep(&mut self, id1: BuildTaskId, id2: BuildTaskId, id3: BuildTaskId) {
        let temp = [id1, id2, id3];
        let collected = temp
            .iter()
            .filter_map(|k| {
                self.map
                    .get(k)
                    .map(|mk| mk.get_step(buildstep::STeX::Pdflatex2))
            })
            .collect::<Vec<_>>();

        for i in collected.iter().enumerate() {
            let x = collected.get(i.0).unwrap();
            if i.0 == 0 {
                let mut y = x.borrow_mut();
                y.add_dependency(id2, collected.get(1).unwrap().clone());
                y.add_dependency(id3, collected.get(2).unwrap().clone());
            } else if i.0 == 1 {
                let mut y = x.borrow_mut();
                y.add_dependency(id3, collected.get(2).unwrap().clone());
            } else {
                let mut y = x.borrow_mut();
                y.add_dependency(id1, collected.get(0).unwrap().clone());
            }
        }
    }
}

pub struct CreateGraphResult(
    pub DiGraph<(BuildTaskId, STeX), ()>,
    pub HashMap<NodeIndex, (BuildTaskId, STeX)>,
);

impl TaskMap<BuildTaskId> {
    pub fn create_graph(&self) -> CreateGraphResult {
        let mut d_g = DiGraph::new();
        let mut graph_nodes = HashMap::new();
        for i in self.map.values() {
            let mut prev_store = None;
            for j in i.get_steps() {
                let node_here = (i.get_id(), j.borrow().get_task_id());
                graph_nodes
                    .entry(node_here)
                    .or_insert_with(|| d_g.add_node(node_here));
                if let Some(m_y) = prev_store {
                    d_g.add_edge(
                        *graph_nodes.get(&m_y).unwrap(),
                        *graph_nodes.get(&node_here).unwrap(),
                        (),
                    );
                }
                for k in j.borrow().dep.iter() {
                    let st = k.step.borrow().get_task_id();
                    graph_nodes
                        .entry((k.buildtaskid, st))
                        .or_insert_with(|| d_g.add_node((k.buildtaskid, st)));

                    d_g.add_edge(
                        *graph_nodes.get(&(k.buildtaskid, st)).unwrap(),
                        *graph_nodes.get(&node_here).unwrap(),
                        (),
                    );
                }
                prev_store = Some(node_here);
            }
        }
        let send_nodes = graph_nodes
            .into_iter()
            .map(|(k, v)| (v, k))
            .collect::<HashMap<_, _>>();
        CreateGraphResult(d_g, send_nodes)
    }
}
