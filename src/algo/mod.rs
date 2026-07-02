// here we store a key from the graph and send its value

use std::collections::{HashMap, HashSet};

use petgraph::{
    Direction::{Incoming, Outgoing},
    algo::kosaraju_scc,
    graph::DiGraph,
    visit::EdgeRef,
};
use tokio::sync::mpsc::Sender;
use tracing::info;

pub trait Finished {
    fn is_finished(&self) -> bool;
}
// (id, step -> Taskstate)

pub struct Scheduler<T: Finished + Clone> {
    graph: DiGraph<T, ()>,
    sender: Sender<T>,
}

impl<T: Finished + Clone> Scheduler<T> {
    pub fn new(sender: Sender<T>) -> Self {
        Self {
            graph: DiGraph::new(),
            sender,
        }
    }

    // first to schedule we need next tasks that are no more zero so lets say I populated the graph now
    // i need to schedule i can schedule like say 4 tasks now I have to wait until I send them
    pub async fn schedule(&mut self) {
        // this is a map to store the incoming edges
        let mut scheduled_set = HashSet::new();
        for i in self.graph.node_indices() {
            let weight = self.graph.node_weight(i).expect("not possible");
            if !weight.is_finished() {
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

        for i in scheduled_set.iter() {
            self.graph.remove_node(*i);
        }
        if scheduled_set.is_empty() {
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
    }
}
