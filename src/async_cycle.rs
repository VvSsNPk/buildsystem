use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
    sync::{Arc, atomic::AtomicUsize},
    thread::sleep,
    time::Duration,
};

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
    deps: Arc<AtomicUsize>,
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
    pub fn new(task: T, deps: usize) -> Self {
        let taski = TaskI::new(task, deps);
        Self(Arc::new(taski))
    }
    pub fn new_with_timer(t: T, n: usize, deps: usize) -> Self {
        let taski = TaskI::new_with_timer(t, n, deps);
        Self(Arc::new(taski))
    }
    fn updat_state(&mut self, state: TaskState) {
        let mut lock = self.0.state.write();
        *lock = state;
    }
}

impl<T: Clone + Hash> TaskI<T> {
    pub fn new(t: T, deps: usize) -> Self {
        let mut rng = rng();
        let n = rng.random_range(2..10);
        Self {
            node: t,
            deps: Arc::new(AtomicUsize::new(deps)),
            _num: n,
            state: RwLock::new(TaskState::Queued),
        }
    }

    pub fn new_with_timer(t: T, n: usize, deps: usize) -> Self {
        Self {
            node: t,
            deps: Arc::new(AtomicUsize::new(deps)),
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
    let tasks: Vec<_> = g.iter().map(|(n, k)| Task::new(*n, k.len())).collect();
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
    println!("finished");
}

// is this the right approach ?
// Strongly conected
// This seems right
// Only for strongly connected components
type Graph<T> = Arc<[(Task<T>, Vec<Task<T>>)]>;

// So this is for cycles but if there are no cycles then ?

pub async fn run_task_queue<T: Clone + Eq + Hash + Send + Sync  + 'static>(
    graph: Graph<T>,
    store: Arc<Mutex<VecDeque<Task<T>>>>,
    semaphore: Arc<Semaphore>,
) {
    // assumption is that store is pre sorted here
    let counter = Arc::new(AtomicUsize::new(0));
    while let Ok(p) = Semaphore::acquire_owned(semaphore.clone()).await {
        let mut task_lock = store.lock();
        // here we need to check the end of the task and if there are no dependencies left then only pop else
        let task = task_lock.iter().last().unwrap();
        let pending = task.0.deps.load(std::sync::atomic::Ordering::Relaxed);
        let str = Arc::clone(&store);
        let g = graph.clone();
        // here the stack is popped, is there a way to pop from front and back asynchronously and getting tasks from front and back ?
        if pending == 0 {
            if let Some(task) = task_lock.pop_back() {
                drop(task_lock);
                let counter_clone = Arc::clone(&counter);
                spawn_blocking(move || {
                    let t = task;
                    counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    sleep(Duration::from_secs(t.0._num as u64));
                    counter_clone.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    let mut to_sort = str.lock();
                    to_sort.make_contiguous().sort_by(|a, b| {
                        let m = find_wrapper_children(&g, a.clone()).len();
                        let n = find_wrapper_children(&g, b.clone()).len();
                        n.cmp(&m)
                    });
                    drop(to_sort);
                    drop(p);
                });
            }
        } else {
            let counter_break = counter.load(std::sync::atomic::Ordering::Relaxed);
            let k = store.lock();
            if counter_break == 0 && k.is_empty() {
                break;
            }
        }
    }
}
