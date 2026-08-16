// The scheduler owns a dependency graph of tasks and drives it forward by
// handing "ready" tasks to the executor and reacting to the state updates
// the executor reports back. Crucially: nodes and edges are never removed
// from the graph. Completion is tracked purely as per-node state
// (`ExecutorState`), which is what "ready" is computed from. This mirrors
// what actually happens on the wire: sending a task on `sender` only means
// "the executor accepted it", not "it ran" - the executor reports Running,
// then Finished/Failed, asynchronously and out of order, via
// `state_updater`. Mutating graph structure on send (the old approach)
// conflated those two things and could unblock a task's dependents before
// the dependency had actually finished.

use std::{collections::HashMap, hash::Hash};

use petgraph::{
    Direction::Incoming,
    algo::kosaraju_scc,
    graph::{DiGraph, NodeIndex},
    stable_graph::StableDiGraph,
    visit::NodeFiltered,
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::rec::{ExecutorState, State};

pub struct Scheduler<T: Clone + Eq + Hash> {
    graph: StableDiGraph<T, ()>,
    // reverse lookup: task identity -> its node, so a `State<T>` coming back
    // from the executor can be applied to the right node.
    index: HashMap<T, NodeIndex>,
    // per-node execution state. Absent == ExecutorState::None (not yet
    // dispatched). This is the thing that changes instead of the graph.
    state: HashMap<NodeIndex, ExecutorState>,
    sender: Sender<T>,
    state_updater: Receiver<State<T>>,
    // number of nodes dispatched (Queued or Running) but not yet
    // Finished/Failed. Used to tell "nothing ready because we're waiting on
    // in-flight work" apart from "nothing ready because we're deadlocked on
    // a cycle".
    in_flight: usize,
}

impl<T: Clone + Eq + Hash> Scheduler<T> {
    pub fn new(sender: Sender<T>, state_updater: Receiver<State<T>>) -> Self {
        Self {
            graph: StableDiGraph::new(),
            index: HashMap::new(),
            state: HashMap::new(),
            sender,
            state_updater,
            in_flight: 0,
        }
    }

    /// Build a scheduler directly from an already-assembled dependency
    /// graph (e.g. `TaskMap::create_graph`'s output), where an edge
    /// `dep -> dependent` means `dep` must finish before `dependent` runs.
    pub fn from_graph(
        graph: DiGraph<T, ()>,
        sender: Sender<T>,
        state_updater: Receiver<State<T>>,
    ) -> Self {
        let mut scheduler = Self::new(sender, state_updater);
        let mut remap = HashMap::new();
        for old_idx in graph.node_indices() {
            let weight = graph
                .node_weight(old_idx)
                .expect("just iterated it")
                .clone();
            remap.insert(old_idx, scheduler.add_task(weight));
        }
        for edge in graph.raw_edges() {
            scheduler
                .graph
                .update_edge(remap[&edge.source()], remap[&edge.target()], ());
        }
        scheduler
    }

    fn get_or_insert(&mut self, task: T) -> NodeIndex {
        if let Some(&idx) = self.index.get(&task) {
            return idx;
        }
        let idx = self.graph.add_node(task.clone());
        self.index.insert(task, idx);
        self.state.insert(idx, ExecutorState::None);
        idx
    }

    /// Register a task with no known dependencies/dependents yet.
    pub fn add_task(&mut self, task: T) -> NodeIndex {
        self.get_or_insert(task)
    }

    /// Record that `dependency` must finish before `dependent` can run.
    /// Idempotent: calling it twice for the same pair doesn't duplicate the
    /// edge.
    pub fn add_dependency(&mut self, dependency: T, dependent: T) {
        let dep_idx = self.get_or_insert(dependency);
        let dependent_idx = self.get_or_insert(dependent);
        self.graph.update_edge(dep_idx, dependent_idx, ());
    }

    fn node_state(&self, idx: NodeIndex) -> ExecutorState {
        self.state.get(&idx).copied().unwrap_or(ExecutorState::None)
    }

    /// Ready = never dispatched, and every dependency has actually
    /// Finished (not just "sent" - Queued/Running don't count).
    fn is_ready(&self, idx: NodeIndex) -> bool {
        self.node_state(idx) == ExecutorState::None
            && self
                .graph
                .neighbors_directed(idx, Incoming)
                .all(|dep| self.node_state(dep) == ExecutorState::Finished)
    }

    fn ready_nodes(&self) -> Vec<NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&i| self.is_ready(i))
            .collect()
    }

    fn all_settled(&self) -> bool {
        self.graph.node_indices().all(|i| {
            matches!(
                self.node_state(i),
                ExecutorState::Finished | ExecutorState::Failed
            )
        })
    }

    /// Send every currently-ready node to the executor. Marks each as
    /// Queued immediately so the next scan never sends it again - this is
    /// the replacement for removing the node from the graph.
    async fn dispatch_ready(&mut self) {
        for idx in self.ready_nodes() {
            let weight = self
                .graph
                .node_weight(idx)
                .expect("just iterated it")
                .clone();
            self.sender
                .send(weight)
                .await
                .expect("executor channel closed");
            self.state.insert(idx, ExecutorState::Queued);
            self.in_flight += 1;
        }
    }

    /// Apply one state update coming back from the executor to the node it
    /// refers to.
    fn apply_state(&mut self, update: State<T>) {
        let Some(&idx) = self.index.get(update.task()) else {
            // Update for a task this scheduler never dispatched - ignore
            // rather than panicking, in case the executor is shared.
            return;
        };
        let new_state = update.taskstate();
        if matches!(new_state, ExecutorState::Finished | ExecutorState::Failed) {
            self.in_flight = self.in_flight.saturating_sub(1);
        }
        self.state.insert(idx, new_state);
    }

    /// Drive every node to Finished/Failed. Dispatches whatever is ready,
    /// waits for the executor to report back, updates state, and repeats.
    /// If we ever end up with nothing ready and nothing in flight while
    /// nodes remain, the remainder is entirely cycles - break one edge's
    /// worth of deadlock by force-dispatching the most-depended-on
    /// undispatched node in the most "downstream" unresolved SCC, then
    /// carry on; its dependents unblock normally once it reports Finished.
    pub async fn schedule(&mut self) {
        loop {
            self.dispatch_ready().await;

            if self.all_settled() {
                return;
            }

            if self.in_flight == 0 {
                if !self.unblock_one_cycle_node().await {
                    // nothing ready, nothing in flight, and no undispatched
                    // node anywhere - genuinely nothing left to do.
                    return;
                }
                continue;
            }

            match self.state_updater.recv().await {
                Some(update) => self.apply_state(update),
                None => return, // executor side is gone
            }
        }
    }

    /// Like `schedule`, but asserts the graph is acyclic instead of trying
    /// to break cycles - use when the caller has already guaranteed there
    /// are none.
    pub async fn schedule_topo(&mut self) {
        loop {
            self.dispatch_ready().await;

            if self.all_settled() {
                return;
            }

            assert!(
                self.in_flight > 0,
                "schedule_topo: stuck with no ready and no in-flight nodes - graph has a cycle"
            );

            match self.state_updater.recv().await {
                Some(update) => self.apply_state(update),
                None => return,
            }
        }
    }

    /// Find the most fundamental unresolved cycle (the SCCs are visited in
    /// the same "most downstream first" order Kahn's algorithm would peel
    /// them off in) and force-dispatch its most-depended-on undispatched
    /// member. Returns false if there was nothing left to unblock.
    async fn unblock_one_cycle_node(&mut self) -> bool {
        // Feed kosaraju_scc a *view* over `self.graph` that skips settled
        // (Finished/Failed) nodes, instead of physically removing them.
        // NodeFiltered hides a node (and every edge touching it) from
        // traversal without mutating the underlying graph at all, so this
        // costs nothing beyond a state lookup per visited node/edge - no
        // rebuild, no index invalidation, and `self.graph` stays the single
        // source of truth for the whole task's lifetime.
        let state = &self.state;
        let pending = NodeFiltered::from_fn(&self.graph, |n| {
            !matches!(
                state.get(&n).copied().unwrap_or(ExecutorState::None),
                ExecutorState::Finished | ExecutorState::Failed
            )
        });
        let sccs = kosaraju_scc(&pending);
        for scc in sccs.iter().rev() {
            let candidate = scc
                .iter()
                .copied()
                .filter(|&n| self.node_state(n) == ExecutorState::None)
                .max_by_key(|&n| self.graph.neighbors_directed(n, Incoming).count());
            if let Some(node) = candidate {
                let weight = self.graph.node_weight(node).expect("in graph").clone();
                self.sender
                    .send(weight)
                    .await
                    .expect("executor channel closed");
                self.state.insert(node, ExecutorState::Queued);
                self.in_flight += 1;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rec::Executor;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    // Runs an Executor on its own task, logging each task id (in the order
    // its blocking work actually completed) to `log`. Returns the join
    // handle so the caller can await clean shutdown after dropping the
    // scheduler (which closes the channel `rx` reads from).
    fn spawn_executor(
        permits: usize,
        rx: Receiver<u32>,
        stx: Sender<State<u32>>,
        log: Arc<Mutex<Vec<u32>>>,
    ) -> tokio::task::JoinHandle<()> {
        let executor = Executor::new(permits, rx, stx);
        tokio::spawn(async move {
            executor
                .run(move |t: u32| {
                    // real (blocking) work, running on tokio's blocking pool
                    std::thread::sleep(Duration::from_millis(5));
                    log.lock().unwrap().push(t);
                    Ok(())
                })
                .await;
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn diamond_dependency_runs_in_dependency_order() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3  (A -> B, A -> C, B -> D, C -> D)
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_dependency(0, 1);
        scheduler.add_dependency(0, 2);
        scheduler.add_dependency(1, 3);
        scheduler.add_dependency(2, 3);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(4, rx, stx, log.clone());

        // If dispatch mutated the graph on *send* instead of on actual
        // completion, 3 could be sent before 1/2's spawn_blocking work had
        // run - this would still terminate but could violate ordering. The
        // timeout guards against the opposite failure mode (deadlock).
        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler should not hang on an acyclic graph");

        drop(scheduler); // closes `tx`, letting the executor's recv loop end
        handle.await.unwrap();

        let log = log.lock().unwrap();
        assert_eq!(log.len(), 4);
        let pos = |t: u32| log.iter().position(|&x| x == t).unwrap();
        assert!(pos(0) < pos(1), "A must finish before B: {log:?}");
        assert!(pos(0) < pos(2), "A must finish before C: {log:?}");
        assert!(pos(1) < pos(3), "B must finish before D: {log:?}");
        assert!(pos(2) < pos(3), "C must finish before D: {log:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_node_cycle_still_completes() {
        // 0 -> 1 -> 0: unresolvable cycle. schedule() must force-dispatch
        // one side instead of deadlocking, and both must still run.
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_dependency(0, 1);
        scheduler.add_dependency(1, 0);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(2, rx, stx, log.clone());

        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler must not deadlock on a cycle");

        drop(scheduler);
        handle.await.unwrap();

        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2);
        assert!(log.contains(&0));
        assert!(log.contains(&1));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cycle_unblocking_ignores_unrelated_finished_nodes() {
        // 10, 11: independent, zero-dep, will settle before the cycle is
        // ever considered. 20 -> 21 -> 22 -> 20: a genuine 3-cycle,
        // unrelated to 10/11. Once 10/11 finish, unblock_one_cycle_node's
        // NodeFiltered view must still find and break the cycle correctly
        // even though the graph also contains settled, unrelated nodes.
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_task(10);
        scheduler.add_task(11);
        scheduler.add_dependency(20, 21);
        scheduler.add_dependency(21, 22);
        scheduler.add_dependency(22, 20);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(4, rx, stx, log.clone());

        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler must not deadlock or stall");

        drop(scheduler);
        handle.await.unwrap();

        let log = log.lock().unwrap();
        assert_eq!(log.len(), 5);
        for t in [10, 11, 20, 21, 22] {
            assert!(log.contains(&t), "task {t} never ran: {log:?}");
        }
    }
}
