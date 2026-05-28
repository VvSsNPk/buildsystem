// just a practice to implement your own types

pub struct VecSet<T> {
    inner: Vec<T>,
}

impl<T> VecSet<T> {
    pub fn new() -> Self {
        Self { inner: vec![] }
    }
}

impl<T> Default for VecSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
