use std::{collections::HashSet, vec};

use petgraph::Graph;

use crate::task::buildtask::BuildTask;

#[macro_export]
macro_rules! make_graph_ai {
    ( $( $e:ident -> [ $($b:ident),* ] ),* $(,)? ) => {{
        let mut g = Vec::new();
        $(
            let mut children = Vec::new();
            $(
                children.push(Node::$b);
            )*
            g.push((Node::$e, children));
        )*
        g
    }};
    ( $( $e:ident -> $b:ident ),* $(,)? ) => {{
        let mut g = Vec::new();
        $(
            g.push((Node::$e, vec![Node::$b]));
        )*
        g
    }};
}

// So we run johson simple cycles and identify the cyles
#[test]
pub fn create_example_graph() {
    let g = [
        (Node::A, vec![Node::B]),
        (Node::B, vec![Node::E, Node::C, Node::K]),
        (Node::C, vec![Node::D, Node::F, Node::G]),
        (Node::D, vec![Node::A]),
        (Node::E, vec![Node::B]),
        (Node::F, vec![Node::C]),
        (Node::G, vec![Node::C, Node::H]),
        (Node::H, vec![Node::C, Node::I]),
        (Node::I, vec![Node::H, Node::M, Node::N]),
        (Node::J, vec![Node::C]),
        (Node::K, vec![Node::L]),
        (Node::L, vec![Node::B, Node::O]),
        (Node::M, vec![Node::I]),
        (Node::N, vec![Node::I]),
        (Node::O, vec![Node::L]),
    ];
    // so rule of thumb is a node either has one node as dependency or more than once if more than one then it participates in more cycles
    // first  pick the node with most urgency why because this reduces the number of times that I need to make this node run
    // We only run it assuming no failure because of missing dependencies
    // Now we run the children i.e  G,F,D since all the nodes in the graph are gauranteed to be in cycles with the actual nodes I dont need to care about anything here
    // only thing now I care is if its immeadiate cycle ? i.e check for the next children of G,F,D if immediate cylce then put the nodes into blocked until this is resolved
    // if not pick the next non immediate cycles and G , D run them and get their children i.e H,A
    // now H and A check again for the next neighbhour because this is important if H encounters C as neighbour dont run it put it in the queue here i.e blocked queue, but what if H participates in another cycles ?
    //

    // run largest node : Node::C
    //
    let x = run(&g, Node::C);
    assert_eq!(
        x,
        vec![
            Node::C,
            Node::D,
            Node::A,
            Node::B,
            Node::E,
            Node::B,
            Node::F,
            Node::G,
            Node::H,
            Node::I,
            Node::H,
            Node::C
        ]
    )
}

pub fn run(g: &[(Node, Vec<Node>)], root: Node) -> Vec<Node> {
    let mut stack = vec![root];
    let mut already_ran = HashSet::new();
    let mut remainder = vec![root];
    let mut ans = vec![];
    while let Some(x) = stack.pop() {
        // run the node x
        ans.push(x);
        if !already_ran.contains(&x) {
            already_ran.insert(x);
            let children = find_wrapper_children(g, x).iter().filter(|n| **n != root);
            for i in children {
                if already_ran.contains(i) {
                    if !remainder.contains(i) {
                        remainder.push(*i);
                    }
                } else {
                    stack.push(*i);
                }
            }
        }
    }
    remainder.reverse();
    ans.extend_from_slice(&remainder);
    ans
}
// A : B C
// B : A D
// C : A
// D : B
// current = 0
// [A]
// x = A
// [A,B,C]
// current = 1
// x = B
// [A, B ,C ,D]
// current = 2
// x = C
// [A,B,C,D]
// current = 3
// x = D
// [A,B,C,D,B]
// current = 4
// continue;
// A to the end
//
// mapping how run2 works
//
#[test]
fn run2_test() {
    let g = [
        (Node::A, vec![Node::B, Node::C]),
        (Node::B, vec![Node::D, Node::A]),
        (Node::C, vec![Node::A]),
        (Node::D, vec![Node::B]),
    ];
    let x = run2(&g, Node::A);
    assert_eq!(
        x,
        vec![Node::A, Node::B, Node::C, Node::D, Node::B, Node::A]
    )
}

/// This algorithm works properly but only for one fix point iteration i.e A -> B -> A
/// what if the iterations are more i.e A -> B -> A -> B -> A -> B
/// what if we flip the edges ? and run it now children for C are B H F J
/// we first pick the node that has max dependencies
/// Now we have to think about running it async
pub fn run2<N: Eq + Clone + Copy>(g: &[(N, Vec<N>)], root: N) -> Vec<N> {
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

// [C]
// current = 0
// x = C
// [C,G,F,D,J]
// current = 1
// x = G
// [C,G,F,D,J,H]
// current = 2
// x = F
// [C,G,F,D,J,H]
// current = 3
// x = D
// [C,G,F,D,J,H,A]
// current = 4
// x = J
// [C,G,F,D,J,H,A]
// current = 5
// x = H
// [C,G,F,D,J,H,A,I]
// current = 6
// x = A
// [C,G,F,D,J,H,A,I,B],
// current = 7
// x = I
// [C,G,F,D,J,H,A,I,B,H,N,M]

#[test]
fn test3() {
    let g = make_graph_ai!(
        A -> [B] ,
        B -> [K,E,C] ,
        C -> [G,F,D,J] ,
        D -> [A] ,
        E -> [B] ,
        F -> [C] ,
        G -> [H],
        H-> [I,C] ,
        I -> [H,N,M,P],
        J -> [C] ,
        K -> [B,L] ,
        L -> [O,B] ,
        M -> [I] ,
        N -> [I], O -> [L],P -> [I] );
    let x = run2(&g, Node::C);
    let y = run(&g, Node::C);
    assert_eq!(x, y)
}

// gets the next deps that it needs to run after running the node :Node
pub fn find_wrapper_children<N: Eq>(g: &[(N, Vec<N>)], node: N) -> &[N] {
    &g.iter().find(|(n, _)| *n == node).unwrap().1
}
// This  function checks whether n is immediate to node i.e A -> B -> A
pub fn is_immeaidate_in_cycle(g: &[(Node, Vec<Node>)], n: Node, node: Node) -> bool {
    find_wrapper_children(g, n).contains(&node)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum Node {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
}

#[macro_export]
macro_rules! make_graph {
    ($($e:ident -> [$($b:ident),*]),*) => {
        let mut g = Vec::new();
        $(
            let mut k = Vec::new();
            $(
                k.push(Node::$b);
            )*
            g.push((Node::$e,k))
        )*
        g
    };
}
