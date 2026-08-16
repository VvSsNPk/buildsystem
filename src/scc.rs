pub struct SccStore<N> {
    graph: Vec<SccNode<N>>,
}

impl<N> SccStore<N> {
    pub fn new() -> Self {
        Self { graph: vec![] }
    }
}

pub enum SccNode<N> {
    Node(N),
    SCC(Vec<N>),
}

impl<N> SccNode<N> {
    pub fn make_query(&self) {
        todo!(
            "use pet Graph G with some implementation which gives me their incoming and out going edges"
        )
    }
}
