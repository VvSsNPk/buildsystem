use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;

use crate::task::buildstep::STeX;
use crate::task::buildtask::BuildTaskId;
use buildsystem::cycle_handler::{find_wrapper_children, run2};
use buildsystem::utils::time::{Delta, Timestamp};
use either::Either;
use indexmap::IndexMap;
use petgraph::Direction::Incoming;
use petgraph::Graph;
use petgraph::graph::node_index;
use petgraph::graphmap::NodeTrait;
use petgraph::visit::{
    DfsEvent, GraphBase, GraphRef, IntoNeighbors, Visitable, depth_first_search,
};
use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::{Dfs, DfsPostOrder, IntoNodeIdentifiers, Reversed, VisitMap},
};

use fixedbitset::FixedBitSet;
pub mod macros;
pub mod task;

fn main() {
    let di_graph = DiGraph::<i32, ()>::from_edges([
        (0, 1),
        (1, 0),
        (0, 2),
        (2, 0),
        (4, 6),
        (0, 6),
        (6, 0),
        (2, 3),
        (3, 2),
        (2, 4),
        (4, 2),
        (6, 4),
    ]);

    run_petgraph(&di_graph, node_index(0));
    // let mut visited: HashSet<NodeIndex> = HashSet::new();
    // let mut finished: Vec<NodeIndex> = vec![];
    // let mut time = 0;
    // depth_first_search(&di_graph, Some(node_index(0)), |e| match e {
    //     DfsEvent::Discover(n, time) => {
    //         println!("first run {:?}", n);
    //     }
    //     DfsEvent::TreeEdge(n, m) => {
    //         //println!("we don't care about this {:?}", m);
    //     }
    //     DfsEvent::BackEdge(n, m) => {
    //         if finished.contains(&m) {
    //             println!("running {:?}", m)
    //         }
    //     }
    //     DfsEvent::CrossForwardEdge(n, m) => {
    //         //println!("we don't care here as well {:?}", m);
    //     }
    //     DfsEvent::Finish(n, time) => {
    //         println!("running in finished {:?}", n);
    //         finished.push(n);
    //     }
    // });

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

pub fn run2_copy<N: Eq + Clone + Copy>(g: &mut [(N, Vec<N>)], root: N) -> Vec<N> {
    let mut current = 0;
    let mut result = vec![root];
    let mut remainder = vec![root];
    while current != result.len() {
        let x = result[current];
        current += 1;
        if result[0..current - 1].contains(&x) {
            continue;
        }
        let f_c = find_wrapper_children(g, x).iter().filter(|n| **n != root);
        for i in f_c {
            if result[0..current - 1].contains(i) {
                if !remainder.contains(i) {
                    remainder.push(*i);
                }
            } else {
                result.push(*i);
            }
        }
    }
    remainder.reverse();
    result.extend_from_slice(&remainder);
    result
}

pub fn run_petgraph<Nt, E>(g: &DiGraph<Nt, E>, root: NodeIndex) {
    let mut stack = vec![root];
    // This is just to check whether its already ran i think ?
    let mut already_ran = HashSet::new();
    let mut final_run = vec![root];

    while let Some(n) = stack.pop() {
        if !already_ran.contains(&n) {
            println!("this is what is ran {:?}", n);
            already_ran.insert(n);
            let children = g.neighbors_directed(n, Incoming).filter(|n| *n != root);
            let mut child2: Vec<_> = children.collect();
            child2.reverse();
            for i in child2 {
                if already_ran.contains(&i) {
                    if !final_run.contains(&i) {
                        final_run.push(i);
                    }
                } else {
                    stack.push(i);
                }
            }
        }
    }
    for i in final_run {
        println!("the final run again {:?}", i);
    }
}
