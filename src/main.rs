use crate::{scc::kosaraju, scc::extract_sub_graph};
use rustworkx_core::connectivity::johnson_simple_cycles;

pub mod macros;
pub mod task;
pub mod scc;

fn main() {
    let x = make_dep!(1 => 2,2 => 3,3=>4,4=>3,3=>5,5=>6,7=>6,8=> 7,9=>6,6=> 13,13=>14,15=>14,14=>12,16=>12,12=>11,11=>10,10=>6);
    //let x = make_dep!(1=>2,2=>3,2=>4,3=>5,3=>6,3=>7,4=>8,4=>9,4=>10);
    //let x = make_dep!(2 => 1,1=>0,0=>2,2=>4,4=>3,3=>2);
    let d_g = x.create_graph();
    let sccs = kosaraju(&d_g.0);
    println!("simple cycles begins here");
    for i in sccs {
        let new_graph = extract_sub_graph(&d_g.0, &i);
        let mut cycles = johnson_simple_cycles(&new_graph.0, None);
        while let Some(k) =  cycles.next(&new_graph.0){
            let k_p = k.iter().filter_map(|nd|new_graph.1.get(nd)).collect::<Vec<_>>();
            println!("{:?}",k_p);
        }
    }
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


