// ignore-compare-mode-chalk

trait Next {
    type Next: Next;
}

struct GetNext<T: Next> { t: T }

impl<T: Next> Next for GetNext<T> {
    //~^ ERROR overflow evaluating the requirement
    type Next = <GetNext<T> as Next>::Next;
    //~^ ERROR overflow evaluating the requirement
}

fn main() {}
