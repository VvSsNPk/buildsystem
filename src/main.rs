use std::collections::{HashMap, VecDeque};

use crate::task::buildtask::BuildTaskId;
use crate::task::buildstep::STeX;
use buildsystem::cycle_handler::run2;
use buildsystem::utils::time::{Delta, Timestamp};
use either::Either;
use indexmap::IndexMap;
use petgraph::Direction::Incoming;
use petgraph::Graph;
use petgraph::algo::TarjanScc;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::{Dfs, DfsPostOrder, IntoNodeIdentifiers, Reversed, VisitMap},
};
use tracing::Level;

pub mod macros;
pub mod task;

fn main() {
    let mut  g = DiGraph::new();
    let n1 = g.add_node(1);
    let n2 = g.add_node(2);
    let n3 = g.add_node(3);
    let n4 = g.add_node(4);

    g.add_edge(n1, n2, ());
    g.add_edge(n2, n3, ());
    g.add_edge(n3, n4, ());
    g.add_edge(n4, n2, ());
    
    let mut tarjan = TarjanScc::default();
    tarjan.run(&g, |x|{
        println!("the scc are {:?}",x);
    });



    // tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    // let mut k = VecDeque::from([3, 5, 7]);
    // k.make_contiguous().sort_by(|x, y| y.cmp(x));
    // println!("{:?}", k);
    // let (scss, t) = measure(|| {
    //     let (x, t) = measure(
    //         || //make_dep!(1 => 2,2 => 3,3=>4,4=>3,3=>5,5=>6,7=>6,8=> 7,9=>6,6=> 13,13=>14,15=>14,14=>12,16=>12,12=>11,11=>10,10=>6),
    //         TaskMap::entirely_random(50000, 10),
    //     );
    //     println!("Inited in {t}");

    //     let (d_g, t) = measure(|| x.create_graph());
    //     println!("Created in {t}");

    //     let (r, t) = measure(|| kosaraju(&d_g.0));
    //     println!("kosaraju: {t}");
    //     r
    // });
    // println!("Total: {t}")
}

pub fn subgraph(
    graph: DiGraph<(BuildTaskId, STeX), ()>,
    scc: Vec<NodeIndex>,
) -> DiGraph<(BuildTaskId, STeX), ()> {
    let mut new_graph = DiGraph::new();
    let map = scc
        .iter()
        .map(|n| (*n, graph.node_weight(*n).unwrap()))
        .collect::<IndexMap<_, _>>();
    let mut node_store = HashMap::new();
    for i in scc.iter() {
        let weight = map.get(i).unwrap();
        let node = graph.neighbors_directed(*i, Incoming);
        let n = *node_store
            .entry(*weight)
            .or_insert_with(|| new_graph.add_node((weight.0, weight.1)));
        for k in node {
            if scc.contains(&k) {
                let weight2 = map.get(&k).unwrap();
                let k = *node_store
                    .entry(*weight2)
                    .or_insert_with(|| new_graph.add_node((weight2.0, weight2.1)));
                new_graph.add_edge(k, n, ());
            }
        }
    }
    new_graph
}

pub fn get_scc_children(
    graph: &DiGraph<(BuildTaskId, STeX), ()>,
    scc: Vec<NodeIndex>,
    node: NodeIndex,
) -> Vec<NodeIndex> {
    graph
        .neighbors_directed(node, petgraph::Direction::Incoming)
        .filter(|x| scc.contains(x))
        .collect()
}

pub fn cycle_resolver(graph: Graph<(BuildTaskId, STeX), ()>, scc: Vec<NodeIndex>) {}

pub fn measure<R>(f: impl FnOnce() -> R) -> (R, Delta) {
    let now = Timestamp::now();
    let r = f();
    let delta = now.since_now();
    (r, delta)
}

#[macro_export]
macro_rules! make_dep{
    ($($f:literal => $g:literal),*) => {
        {
        let mut taskmap = $crate::task::TaskMap::new();
        $(
            taskmap.create_link($f,$g);
        )*
        taskmap
        }
    };
}

/// ?????
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

// This is kosaraju scc algorithm (what does it do? )
// Here instead of giving node index we give <something else>(???)
pub fn kosaraju(g: &DiGraph<(BuildTaskId, STeX), ()>) -> Vec<NodeIndex> {
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
            let mut x = vec![];
            let mut root = (*scc.first().unwrap(), 0);
            for i in scc.iter() {
                let child = g
                    .neighbors_directed(*i, petgraph::Direction::Outgoing)
                    .filter(|n| scc.contains(n))
                    .collect::<Vec<_>>();
                if child.len() > root.1 {
                    root = (*i, child.len());
                }
                x.push((*i, child))
            }
            let x = run2(&mut x, root.0);
            sccs.extend_from_slice(&x);
        } else {
            sccs.push(*scc.first().unwrap());
        }
    }
    sccs
}
