// pretty-expanded FIXME #23616

trait PoolManager {
    type C;
    fn dummy(&self) { }
}

struct InnerPool<M> {
    manager: M,
}

impl<M> InnerPool<M> where M: PoolManager {}

fn main() {}
