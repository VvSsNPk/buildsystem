use std::{collections::HashSet, hash::Hash, sync::Arc, thread::sleep, time::Duration};

use parking_lot::{Mutex, RwLock};
use rand::{RngExt, rng};
use tokio::{
    sync::{
        Semaphore,
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
    task::spawn_blocking,
};

use crate::{
    cycle_handler::{Node, find_wrapper_children},
    task,
};
#[derive(Debug)]
pub struct TaskI<T: Clone + Hash> {
    node: T,
    // this should be rng
    _num: usize,
    state: RwLock<TaskState>,
}
// BuildTask : Box<[BuildStep(RwLock)]>
// BuildTask : BuildStep  -> Build -> [TaskState] : [TaskState]
impl<T: Clone + Hash> Hash for TaskI<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node.hash(state);
    }
}
impl<T: Clone + PartialEq + Hash> PartialEq for TaskI<T> {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}
impl<T: Clone + Eq + Hash> Eq for TaskI<T> {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Task<T: Clone + Eq + Hash>(Arc<TaskI<T>>);
impl<T: Clone + Hash + Eq> Task<T> {
    pub fn new(task: T) -> Self {
        let taski = TaskI::new(task);
        Self(Arc::new(taski))
    }
    pub fn new_with_timer(t: T, n: usize) -> Self {
        let taski = TaskI::new_with_timer(t, n);
        Self(Arc::new(taski))
    }
    fn updat_state(&mut self, state: TaskState) {
        let mut lock = self.0.state.write();
        *lock = state;
    }
}

impl<T: Clone + Hash> TaskI<T> {
    pub fn new(t: T) -> Self {
        let mut rng = rng();
        let n = rng.random_range(2..10);
        Self {
            node: t,
            _num: n,
            state: RwLock::new(TaskState::Queued),
        }
    }

    pub fn new_with_timer(t: T, n: usize) -> Self {
        Self {
            node: t,
            _num: n,
            state: RwLock::new(TaskState::Queued),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Done,
    Running,
    Finished,
    Queued,
}
#[tokio::test(flavor = "multi_thread")]
async fn create_graph() {
    let g = [
        (Node::A, vec![Node::B]),
        (Node::B, vec![Node::E, Node::C, Node::K]),
        (Node::C, vec![Node::D, Node::F, Node::G]),
        (Node::D, vec![Node::A]),
        (Node::E, vec![Node::B]),
        (Node::F, vec![Node::C]),
        (Node::G, vec![Node::C, Node::H]),
        (Node::H, vec![Node::C, Node::I]),
        (Node::I, vec![Node::H, Node::M, Node::N]),
        (Node::J, vec![Node::C]),
        (Node::K, vec![Node::L]),
        (Node::L, vec![Node::B, Node::O]),
        (Node::M, vec![Node::I]),
        (Node::N, vec![Node::I]),
        (Node::O, vec![Node::L]),
    ];
    let tasks: Vec<_> = g.iter().map(|(n, _)| Task::new(*n)).collect();
    let mut store = Vec::new();
    for i in g {
        let root = tasks.iter().find(|k| k.0.node == i.0).unwrap().clone();
        let mut store2 = Vec::new();
        for j in i.1 {
            let child = tasks.iter().find(|k| k.0.node == j).unwrap().clone();
            store2.push(child);
        }
        store.push((root, store2));
    }
    let x = tasks.iter().find(|r| r.0.node == Node::C).unwrap();
    let g: Graph<Node> = store.into();
    let sem = Arc::new(Semaphore::new(4));
    let (rc, mut sc) = tokio::sync::mpsc::unbounded_channel();
    let visited = Arc::new(Mutex::new(HashSet::new()));
    let cycle = Arc::new(Mutex::new(HashSet::new()));
    run2(g, x.clone(), sem, rc.clone(), visited, &mut sc, cycle).await;
    println!("finished");
}

// is this the right approach ?
// Strongly conected
// This seems right
// Only for strongly connected components
type Graph<T> = Arc<[(Task<T>, Vec<Task<T>>)]>;

pub async fn run<T: Clone + Eq + Hash + Send + Sync + 'static>(
    g: Graph<T>,
    root: Task<T>,
    sem: Arc<Semaphore>,
    stack: Arc<Mutex<Vec<Task<T>>>>,
    visited: Arc<Mutex<HashSet<Task<T>>>>,
    // Here the problem is that the tail remainder has an order and we need to run the things in that order
    cycle_tail: Arc<Mutex<Vec<Task<T>>>>,
) {
    // I did this becasue the stack is shared across threads because used in spawn_blocking
}

async fn run2<T: Clone + Eq + Hash + Send + Sync + 'static>(
    g: Graph<T>,
    root: Task<T>,
    sem: Arc<Semaphore>,
    sender: UnboundedSender<(bool, Task<T>)>,
    visited: Arc<Mutex<HashSet<Task<T>>>>,
    rc: &mut UnboundedReceiver<(bool, Task<T>)>,
    second_run: Arc<Mutex<HashSet<Task<T>>>>,
) {
    while let Some((b, x)) = rc.recv().await {
        if let Ok(permit) = Semaphore::acquire_owned(sem.clone()).await {
            // do some workhere
            let y = root.0.node.clone();
            let graph = g.clone();
            let sender = sender.clone();
            let vis = visited.clone();
            let sc = second_run.clone();
            spawn_blocking(move || {
                let mut lc = vis.lock();
                if !lc.contains(&x) {
                    lc.insert(x.clone());
                    drop(lc);
                    // some work is done we do no care about failure
                    sleep(Duration::from_secs(x.0._num as u64));
                    if b {
                        let children = find_wrapper_children(&graph, x)
                            .iter()
                            .filter(|n| n.0.node != y);
                        let s = sender;
                        let second_acquire = vis.lock();
                        let another = sc.lock();
                        for i in children {
                            if !second_acquire.contains(i) {
                                if !another.contains(i) {
                                    s.send((false, i.clone())).expect("unable to send task");
                                }
                            } else {
                                s.send((true, i.clone())).expect("error");
                            }
                        }
                        drop(second_acquire);
                        drop(another);
                        drop(s);
                    }
                } else {
                    drop(sender);
                }

                drop(permit)
            });
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fixpoint_parallel_algorithm() {}
