// here we store a key from the graph and send its value

use std::collections::HashSet;

use petgraph::{
    Direction::{Incoming, Outgoing},
    algo::kosaraju_scc,
    graph::NodeIndex,
    stable_graph::StableDiGraph,
    visit::{Dfs, EdgeRef},
};
use tokio::sync::mpsc::Sender;

pub trait Finished {
    fn is_finished(&self) -> bool;
}
// (id, step -> Taskstate)

pub struct Scheduler<T: Finished + Clone> {
    graph: StableDiGraph<T, ()>,
    sender: Sender<T>,
}

impl<T: Finished + Clone> Scheduler<T> {
    pub fn new(sender: Sender<T>) -> Self {
        Self {
            graph: StableDiGraph::new(),
            sender,
        }
    }

    pub async fn schedule_scc(&mut self, scc: &[NodeIndex]) {
        // first the node is a single node i.e length is 1 then we just check its incoming edges and
        // schedule
        // here we assume that the scc schedule will only be called after is_scc_explicit is called
        let max_node = scc
            .iter()
            .max_by(|x, y| {
                let m = self.graph.neighbors_directed(**x, Incoming).count();
                let n = self.graph.neighbors_directed(**y, Incoming).count();
                m.cmp(&n)
            })
            .expect("scc contains node that is not in the graph");
        let weight = self.graph.node_weight(*max_node).expect("impossible");
        self.sender
            .send(weight.clone())
            .await
            .expect("reciever closed ?");
        let edges_to_remove: Vec<_> = self
            .graph
            .edges_directed(*max_node, Outgoing)
            .map(|e| e.id())
            .collect();
        for i in edges_to_remove {
            self.graph.remove_edge(i);
        }
    }

    // we don't remove the nodes or edges from the graph but be more clever use dfs to do scc
    pub async fn schedule_scc_again(&self, scc: &[NodeIndex]) {
        let max_node = scc
            .iter()
            .max_by(|x, y| {
                let m = self.graph.neighbors_directed(**x, Incoming).count();
                let n = self.graph.neighbors_directed(**y, Incoming).count();
                m.cmp(&n)
            })
            .expect("scc contains node that is not in the graph");
        // clearly i need to rethink my algorithm and use petgraph api to implement it

        let mut stack = vec![*max_node];
        while let Some(x) = stack.pop() {
            let weighted = self.graph.node_weight(x).unwrap();
        }
    }

    // This computes assuming the single SCC as a single node and total number of incoming edges
    pub fn get_weights_for_scc(&self, scc: &[NodeIndex]) -> usize {
        // here we query the graph to check for incoming edges in SCC and then we count or filter
        // out the edges that are part of the scc on each of incoming nodes and then we get the
        // total node count which can be used to treat the entire SCC as a single node in the graph
        scc.iter()
            .map(|n| {
                self.graph
                    .neighbors_directed(*n, Incoming)
                    .filter(|n| scc.contains(n))
                    .count()
            })
            .sum()
    }

    pub fn is_scc_explicit(&self, scc: &[NodeIndex]) -> bool {
        self.get_weights_for_scc(scc) == 0
    }

    // first to schedule we need next tasks that are no more zero so lets say I populated the graph now
    // i need to schedule i can schedule like say 4 tasks now I have to wait until I send them
    pub async fn schedule(&mut self) {
        // if schedule_set is empty i.e there are no nodes incoming having 0 edges then we run kosaraju_scc
        // here there is  a problem we are running kosaraju multiple times each call of
        // schedule how to avoid that ? since graph reduces each time is it neccessary to call
        // kosaraju here ?
        let sccs = kosaraju_scc(&self.graph);

        for i in sccs.iter().rev() {
            if i.len() == 1 {
                let neighbhours = self.graph.neighbors_directed(i[0], Incoming);

                if neighbhours.count() == 0 {
                    let weight = self.graph.node_weight(i[0]).expect("impossible");
                    self.sender
                        .send(weight.clone())
                        .await
                        .expect("reciver closed ?");
                    self.graph.remove_node(i[0]);
                }
            } else {
                let all_nodes: usize = i
                    .iter()
                    .map(|n| {
                        self.graph
                            .neighbors_directed(*n, Incoming)
                            .filter(|k| !i.contains(k))
                            .count()
                    })
                    .sum();
                if all_nodes == 0 {
                    let x = *i
                        .iter()
                        .max_by(|n1, n2| {
                            let n = self.graph.neighbors_directed(**n1, Incoming).count();
                            let m = self.graph.neighbors_directed(**n2, Incoming).count();
                            n.cmp(&m)
                        })
                        .expect("impossible");
                    let weight = self.graph.node_weight(x).expect("impossible");
                    self.sender
                        .send(weight.clone())
                        .await
                        .expect("reciever full ?");
                    let neigbhours = self.graph.neighbors_directed(x, Outgoing);
                    let mut to_remove = HashSet::new();
                    for k in neigbhours {
                        let ed = self.graph.edges_connecting(x, k);
                        for i in ed {
                            if i.source() == x && i.target() == k {
                                to_remove.insert(i.id());
                            }
                        }
                    }
                    for i in to_remove {
                        self.graph.remove_edge(i);
                    }
                }
            }
        }
    }

    pub async fn schedule_topo(&mut self) {
        // this hashset is used to later remove nodes from graph
        let mut scheduled_set = HashSet::new();

        // we get all nodes check as iterator using .node_indices()
        for i in self.graph.node_indices() {
            let weight = self.graph.node_weight(i).expect("not possible");

            // we get node weight and check if its not finished that is task is still pending
            if !weight.is_finished() {
                // we get neighbhours that are incoming and check whether all are 0
                // if its so then just send it to the executor which lives some where else holds
                // permits to schedule the tasks and update its state
                let in_edges = self.graph.neighbors_directed(i, Incoming).count();
                if in_edges == 0 {
                    self.sender
                        .send(weight.clone())
                        .await
                        .expect("reciever closed ?");
                    scheduled_set.insert(i);
                }
            }
        }

        // here we use the schedule_set to remove the nodes from the graph so late the graph becomes
        // smaller to run kosaraju_scc
        for i in scheduled_set.iter() {
            self.graph.remove_node(*i);
        }
    }
}
