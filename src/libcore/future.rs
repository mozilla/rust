/*!
 * A type representing values that may be computed concurrently and
 * operations for working with them.
 *
 * # Example
 *
 * ~~~
 * let delayed_fib = future::spawn {|| fib(5000) };
 * make_a_sandwich();
 * io::println(#fmt("fib(5000) = %?", delayed_fib.get()))
 * ~~~
 */

import either::either;

export future;
export extensions;
export from_value;
export from_port;
export from_fn;
export get;
export with;
export spawn;

/// The future type
enum future<A> = {
    mut v: either<@A, fn@() -> A>
};

/// Methods on the `future` type
impl extensions<A:copy send> for future<A> {

    fn get() -> A {
        //! Get the value of the future

        get(self)
    }

    fn with<B>(blk: fn(A) -> B) -> B {
        //! Work with the value without copying it

        with(self, blk)
    }
}

fn from_value<A>(+val: A) -> future<A> {
    /*!
     * Create a future from a value
     *
     * The value is immediately available and calling `get` later will
     * not block.
     */

    future({
        mut v: either::left(@val)
    })
}

fn from_port<A:send>(-port: comm::port<A>) -> future<A> {
    /*!
     * Create a future from a port
     *
     * The first time that the value is requested the task will block
     * waiting for the result to be received on the port.
     */

    do from_fn || {
        comm::recv(port)
    }
}

fn from_fn<A>(f: fn@() -> A) -> future<A> {
    /*!
     * Create a future from a function.
     *
     * The first time that the value is requested it will be retreived by
     * calling the function.  Note that this function is a local
     * function. It is not spawned into another task.
     */

    future({
        mut v: either::right(f)
    })
}

fn spawn<A:send>(+blk: fn~() -> A) -> future<A> {
    /*!
     * Create a future from a unique closure.
     *
     * The closure will be run in a new task and its result used as the
     * value of the future.
     */

    let mut po = comm::port();
    let ch = comm::chan(po);
    do task::spawn || {
        comm::send(ch, blk())
    };
    from_port(po)
}

fn get<A:copy>(future: future<A>) -> A {
    //! Get the value of the future

    do with(future) |v| { v }
}

fn with<A,B>(future: future<A>, blk: fn(A) -> B) -> B {
    //! Work with the value without copying it

    let v = alt copy future.v {
      either::left(v) { v }
      either::right(f) {
        let v = @f();
        future.v = either::left(v);
        v
      }
    };
    blk(*v)
}

#[test]
fn test_from_value() {
    let f = from_value("snail");
    assert get(f) == "snail";
}

#[test]
fn test_from_port() {
    let po = comm::port();
    let ch = comm::chan(po);
    comm::send(ch, "whale");
    let f = from_port(po);
    assert get(f) == "whale";
}

#[test]
fn test_from_fn() {
    let f = fn@() -> str { "brail" };
    let f = from_fn(f);
    assert get(f) == "brail";
}

#[test]
fn test_iface_get() {
    let f = from_value("fail");
    assert f.get() == "fail";
}

#[test]
fn test_with() {
    let f = from_value("nail");
    assert with(f, |v| v) == "nail";
}

#[test]
fn test_iface_with() {
    let f = from_value("kale");
    assert f.with(|v| v) == "kale";
}

#[test]
fn test_spawn() {
    let f = spawn(|| "bale");
    assert get(f) == "bale";
}

#[test]
#[should_fail]
#[ignore(cfg(target_os = "win32"))]
fn test_futurefail() {
    let f = spawn(|| fail);
    let _x: str = get(f);
}
