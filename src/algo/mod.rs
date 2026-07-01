
// here we store a key from the graph and send its value

use std::collections::HashMap;

use petgraph::{Direction::Incoming, graph::DiGraph};
use tokio::sync::mpsc::Sender;


pub trait Finished{
    fn is_finished(&self) -> bool;
}



pub struct Scheduler<T:Finished + Clone>{
    graph : DiGraph<T,()>,
    sender : Sender<T>,
}


impl <T:Finished + Clone> Scheduler<T>{
    pub fn new(sender : Sender<T>) -> Self{
        Self{
            graph : DiGraph::new(),
            sender,
        }
    }


// first to schedule we need next tasks that are no more zero so lets say I populated the graph now
// i need to schedule i can schedule like say 4 tasks now I have to wait until I send them
    pub async  fn schedule(&mut self) {
        // this is a map to store the incoming edges
        let mut store = HashMap::new();
        for i in self.graph.node_indices(){

            let weight = self.graph.node_weight(i).expect("not possible");
            if !weight.is_finished(){
            let in_edges = self.graph.neighbors_directed(i, Incoming).count();
            store.insert(i, in_edges);
            } 
        }

        let mut total_scheduled = 0;
        for (i,j) in store.iter(){
           if *j == 0{
                let weight = self.graph.node_weight(*i).expect("impossible");     
                self.sender.send(weight.clone()).await.expect("channel closed ?");
                total_scheduled += 1;
           } 
        }

        if total_scheduled ==0 {
            todo!("here we do other kosaraju and stuff to process the graph");
        }
    }
}
