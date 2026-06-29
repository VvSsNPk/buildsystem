use std::{
    collections::{HashSet, VecDeque},
    fmt::Debug,
    hash::Hash,
    hint::unreachable_unchecked,
    sync::{Arc, atomic::AtomicUsize},
    time::Duration,
    vec,
};

use parking_lot::{Mutex, RwLock};
use rand::{RngExt, rng};
use tokio::{
    spawn,
    sync::{
        OwnedSemaphorePermit, Semaphore,
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
    task::spawn_blocking,
};
use tracing::{Level, info};

use crate::{
    cycle_handler::{Node, find_wrapper_children},
    task,
};
#[derive(Debug)]
pub struct TaskI<T: Clone + Hash> {
    node: T,
    // this should be rng
    unbuilt_dependencies: Arc<AtomicUsize>,
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
        let n = rng.random_range(1..4);
        Self {
            node: t,
            unbuilt_dependencies: Arc::new(AtomicUsize::new(deps)),
            _num: n,
            state: RwLock::new(TaskState::Queued),
        }
    }

    pub fn new_with_timer(t: T, n: usize, deps: usize) -> Self {
        Self {
            node: t,
            unbuilt_dependencies: Arc::new(AtomicUsize::new(deps)),
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
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let g = [
        (Node::A, vec![Node::B, Node::C]),
        (Node::B, vec![Node::D]),
        (Node::C, vec![Node::E]),
        (Node::D, vec![]),
        (Node::E, vec![]),
        // (Node::A, vec![Node::B]),
        // (Node::B, vec![Node::E, Node::C, Node::K]),
        // (Node::C, vec![Node::D, Node::F, Node::G]),
        // (Node::D, vec![Node::A]),
        // (Node::E, vec![Node::B]),
        // (Node::F, vec![Node::C]),
        // (Node::G, vec![Node::C, Node::H]),
        // (Node::H, vec![Node::C, Node::I]),
        // (Node::I, vec![Node::H, Node::M, Node::N]),
        // (Node::J, vec![Node::C]),
        // (Node::K, vec![Node::L]),
        // (Node::L, vec![Node::B, Node::O]),
        // (Node::M, vec![Node::I]),
        // (Node::N, vec![Node::I]),
        // (Node::O, vec![Node::L]),
    ];
    let mut tasks: Vec<_> = g.iter().map(|(n, k)| Task::new(*n, k.len())).collect();
    info!("the tasks are {:?}", tasks);
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
    let g: DepencyMap<Node> = Arc::new(Mutex::new(store));
    let store = Arc::new(Mutex::new(tasks.into()));
    let sem = Semaphore::new(2);
    info!("now running");
    run_task_queue(g, store, Arc::new(sem)).await;
    println!("finished");
}

// is this the right approach ?
// Strongly conected
// This seems right
// Only for strongly connected components
type DepencyMap<T> = Arc<Mutex<Vec<(Task<T>, Vec<Task<T>>)>>>;

// So this is for cycles but if there are no cycles then ?

pub async fn run_task_queue<T: Clone + Eq + Debug + Hash + Send + Sync + 'static>(
    graph: DepencyMap<T>,
    queue: Arc<Mutex<VecDeque<Task<T>>>>, // <- turns out, this being a VecDeque doesn't buy us anything, because .make_contiguous just "turns it into a Vec" every time anyway
    semaphore: Arc<Semaphore>,
) {
    // assumption is that store is pre sorted here
    info!("executing");
    let counter = Arc::new(AtomicUsize::new(0));
    while let Ok(permit) = Semaphore::acquire_owned(semaphore.clone()).await {
        let mut queue_lock = queue.lock();
        // here we need to check the end of the task and if there are no dependencies left then only pop else
        // Constantly checks the queue for some items until the semaphore permits are there
        // What do we need here ? we need some kind of mechanism that can track time because if there are items then we need to check when they are executed
        let Some(task) = queue_lock.iter().last() else {
            let counter_break = counter.load(std::sync::atomic::Ordering::Relaxed);
            if counter_break == 0 {
                break;
            }
            // better: actually wait for a task to complete; e.g. send a cheap notification via a channel, set a flag, whatever
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            continue;
        };
        let pending = task
            .0
            .unbuilt_dependencies
            .load(std::sync::atomic::Ordering::Relaxed);
        if pending > 0 {
            //info!("the last thing is {} ", pending);
            let counter_break = counter.load(std::sync::atomic::Ordering::Relaxed);
            if counter_break == 0 {
                drop(queue_lock);
                todo!("Cycle!")
            } else {
                // better: actually wait for a task to complete; e.g. send a cheap notification via a channel, set a flag, whatever
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        }
        // here the stack is popped, is there a way to pop from front and back asynchronously and getting tasks from front and back ?
        // pending is last element of the queue we are sorting the queue all the time so it is possible that the last element is not 0 then there are cycles maybe ?
        let Some(task) = queue_lock.pop_back() else {
            // SAFETY: we know queue_lock.iter().last().is_some()
            unsafe { unreachable_unchecked() }
        };
        info!("the task popped {:?}", task);
        drop(queue_lock);

        let counter = counter.clone();
        let graph = graph.clone();
        let queue = queue.clone();
        // we move the data inside the thread
        spawn_blocking(move || run_task(graph, queue, counter, task, permit));
    }
}

enum ChannelMessage {
    Task(task),
    NoTasks,
}

fn run_task<T: Clone + Eq + std::hash::Hash + std::fmt::Debug>(
    graph: DepencyMap<T>,
    queue: Arc<Mutex<VecDeque<Task<T>>>>,
    counter: Arc<std::sync::atomic::AtomicUsize>,
    task: Task<T>,
    permit: OwnedSemaphorePermit,
) {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // TEST ------------------------------------------------------------------
    let now = std::time::Instant::now();
    info!("time is {:?}", now);
    //let child = find_wrapper_children(&mut gk, t.clone());
    let t1 = now.elapsed();
    info!("started the task {:?} at time {} secs", task, t1.as_secs());
    std::thread::sleep(Duration::from_secs(task.0._num as u64));
    let t2 = now.elapsed();
    info!("finishing the task in time {}", t2.as_secs());
    // -----------------------------------------------------------------------
    counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    let mut queue_lock = queue.lock();
    let mut graph_lock = graph.lock();
    let tasks = get_children(&mut graph_lock, &task);
    for i in tasks {
        i.0.unbuilt_dependencies
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
    info!("sorting here");
    // v^ just swap with the last task that has the previous unbuilt-value
    queue_lock.make_contiguous().sort_by(|a, b| {
        let m = find_wrapper_children(&mut graph_lock, a.clone()).len();
        let n = find_wrapper_children(&mut graph_lock, b.clone()).len();
        n.cmp(&m)
    });
    // ^option 1 --------------------------------------------------------------
    let tasks = get_children(&mut graph_lock, &task);
    if tasks.is_empty() {
        channel_sender.send(NoTasks);
    } else {
        for i in tasks {
            if i.0.unbuilt_dependencies.fetch_sub(1, order) == 1 {
                channel_sender.send(Task(i.clone()))
            }
        }
    }
    // alternative: use channel, then all of this is irrrelephant
    drop(queue_lock);
    info!("dropped sort ");
    drop(graph_lock);
    info!("dropped gk");

    drop(permit);
    info!("dropped p");
}

pub fn get_children<'a, T: Clone + Eq + Hash>(
    graph: &'a [(Task<T>, Vec<Task<T>>)],
    task: &Task<T>,
) -> impl Iterator<Item = &'a Task<T>> {
    graph
        .iter()
        .filter(|(_, t)| t.contains(task))
        .map(|(t, _)| t)
}
