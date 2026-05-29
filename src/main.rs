use crate::task::buildtask::BuildTaskId;
use crate::task::{TaskMap, buildstep::STeX};
use buildsystem::utils::time::{Delta, Timestamp};
use either::Either;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::{Dfs, DfsPostOrder, IntoNodeIdentifiers, Reversed, VisitMap},
};

pub mod macros;
pub mod task;

fn main() {
    let (scss, t) = measure(|| {
        let (x, t) = measure(
            || //make_dep!(1 => 2,2 => 3,3=>4,4=>3,3=>5,5=>6,7=>6,8=> 7,9=>6,6=> 13,13=>14,15=>14,14=>12,16=>12,12=>11,11=>10,10=>6),
            // TaskMap::create_map(10000),
            make_dep!(1=>2)
        );
        println!("Inited in {t}");

        //let x = make_dep!(1=>2,2=>3,2=>4,3=>5,3=>6,3=>7,4=>8,4=>9,4=>10);
        //let x = make_dep!(2 => 1,1=>0,0=>2,2=>4,4=>3,3=>2);
        let (d_g, t) = measure(|| x.create_graph());
        println!("Created in {t}");
        // ?????
        let (r, t) = measure(|| kosaraju(&d_g.0));
        println!("kosaraju: {t}");
        r
    });
    /*for i in sccs {
        let k_p = i.iter().filter_map(|nd| d_g.1.get(nd)).collect::<Vec<_>>();
        println!("{:?}", k_p);
    }*/
    println!("Total: {t}")
}

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
