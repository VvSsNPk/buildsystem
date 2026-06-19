use std::collections::HashMap;

use either::Either;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::{Dfs, DfsPostOrder, IntoNodeIdentifiers, Reversed, VisitMap},
    Direction,
};

use crate::task::{buildstep::STeX, buildtask::BuildTaskId};

pub struct SCC<N>(Either<N, Vec<N>>);

impl<N> SCC<N> {
    pub fn new(n: N) -> Self {
        Self(Either::Left(n))
    }

    pub fn new_cycle(n: Vec<N>) -> Self {
        Self(Either::Right(n))
    }
}

// This causes error when the cycle size is less than one
impl<N> std::ops::Deref for SCC<N> {
    type Target = N;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            Either::Left(x) => x,
            Either::Right(x) => x.first().expect("should be atleast 1 size"),
        }
    }
}

pub fn extract_sub_graph(
    g: &DiGraph<(BuildTaskId, STeX), ()>,
    nodes: &[NodeIndex],
) -> (DiGraph<(BuildTaskId, STeX), ()>,HashMap<NodeIndex,(BuildTaskId,STeX)>) {
    let mut new_graph_to_send = DiGraph::new();
    let mut store = HashMap::new();
    for i in nodes.iter() {
        let node_weight = g
            .node_weight(*i)
            .expect("impossible that this does not exist");
        let ent1 = *store
            .entry(node_weight)
            .or_insert_with(|| {
                new_graph_to_send.add_node((node_weight.0, node_weight.1))
            });
        let neighbours = g.neighbors_directed(*i, Direction::Incoming);
        for j in neighbours {
            if nodes.contains(&j) {
                let node_weight2 = g.node_weight(j).expect("this is also impossible");
                let ent2 = *store
                    .entry(node_weight2)
                    .or_insert_with(|| new_graph_to_send.add_node((node_weight2.0, node_weight2.1)));
                new_graph_to_send.add_edge(ent1, ent2, ());
            }
        }
    }
    let to_send = store.iter().map(|(k,l)|(l.clone(),(k.0,k.1))).collect::<HashMap<NodeIndex,(BuildTaskId,STeX)>>();
    (new_graph_to_send,to_send)
}

// This is kosaraju scc algorithm
// Here instead of giving node index we give something else
pub fn kosaraju(g: &DiGraph<(BuildTaskId, STeX), ()>) -> Vec<Vec<NodeIndex>> {
    let mut dfs = DfsPostOrder::empty(g);
    let mut finish_order = Vec::new();
    for i in g.node_identifiers() {
        if dfs.discovered.is_visited(&i) {
            continue;
        }
        dfs.move_to(i);
        while let Some(nx) = dfs.next(Reversed(g)) {
            finish_order.push(nx);
        }
    }

    let mut dfs = Dfs::from_parts(dfs.stack, dfs.discovered);
    dfs.reset(g);
    let mut sccs = Vec::new();
    for i in finish_order.into_iter().rev() {
        if dfs.discovered.is_visited(&i) {
            continue;
        }
        dfs.move_to(i);
        let mut scc = Vec::new();
        while let Some(nx) = dfs.next(g) {
            scc.push(nx)
        }
        if scc.len() > 1 {
            let scc_clone = scc.clone();
            scc.sort_by(|n1, n2| {
                let childern = g
                    .neighbors_directed(*n1, petgraph::Direction::Incoming)
                    .filter(|n| scc_clone.contains(n))
                    .count();
                let childern2 = g
                    .neighbors_directed(*n2, petgraph::Direction::Incoming)
                    .filter(|n| scc_clone.contains(n))
                    .count();
                childern2.cmp(&childern)
            });
        }
        sccs.push(scc);
    }
    sccs
}
