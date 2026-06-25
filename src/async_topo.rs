// trying to write async topological sort algorithm

use indexmap::IndexMap;
use petgraph::visit::{IntoNeighborsDirected, IntoNodeIdentifiers, VisitMap};
use std::{collections::VecDeque, hash::Hash};

pub struct TopoKahn<N, VM> {
    to_visit: VecDeque<N>,
    visited: VM,
    in_degree: IndexMap<N, usize>,
}

impl<N, VM> TopoKahn<N, VM>
where
    N: Copy + Eq + Hash,
    VM: VisitMap<N> + Default,
{
    pub fn new<G>(g: G) -> Self
    where
        G: IntoNeighborsDirected<NodeId = N> + IntoNodeIdentifiers<NodeId = N>,
    {
        let mut in_degree = IndexMap::default();
        let mut tovisit = VecDeque::new();
        for node in g.node_identifiers() {
            let mut degree = 0;
            for _ in g.neighbors_directed(node, petgraph::Direction::Incoming) {
                degree += 1;
            }
            in_degree.insert(node, degree);
            if degree == 0 {
                tovisit.push_back(node);
            }
        }
        Self {
            to_visit: tovisit,
            visited: VM::default(),
            in_degree,
        }
    }
}
