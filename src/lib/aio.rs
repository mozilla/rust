import task;
import vec;

import comm;
import comm::{chan, port, send, recv};
import net;

native "c-stack-cdecl" mod rustrt {
    type socket;
    type server;
    fn aio_init();
    fn aio_run();
    fn aio_stop();
    fn aio_connect(host: *u8, port: int, connected: chan<socket>);
    fn aio_serve(host: *u8, port: int, acceptChan: chan<socket>) -> server;
    fn aio_writedata(s: socket, buf: *u8, size: uint, status: chan<bool>);
    fn aio_read(s: socket, reader: chan<[u8]>);
    fn aio_close_server(s: server, status: chan<bool>);
    fn aio_close_socket(s: socket);
    fn aio_is_null_client(s: socket) -> bool;
}

// FIXME: these should be unsafe pointers or something, but those aren't
// currently in the sendable kind, so we'll unsafely cast between ints.
type server = rustrt::server;
type client = rustrt::socket;
tag pending_connection { remote(net::ip_addr, int); incoming(server); }

tag socket_event { connected(client); closed; received([u8]); }

tag server_event { pending(chan<chan<socket_event>>); }

tag request {
    quit;
    connect(pending_connection, chan<socket_event>);
    serve(net::ip_addr, int, chan<server_event>, chan<server>);
    write(client, [u8], chan<bool>);
    close_server(server, chan<bool>);
    close_client(client);
}

type ctx = chan<request>;

fn ip_to_sbuf(ip: net::ip_addr) -> *u8 unsafe {

    // FIXME: This is broken. We're creating a vector, getting a pointer
    // to its buffer, then dropping the vector. On top of that, the vector
    // created by str::bytes is not null-terminated.
    vec::to_ptr(str::bytes(net::format_addr(ip)))
}

fn# connect_task(args: (net::ip_addr, int, chan<socket_event>)) {
    let (ip, portnum, evt) = args;
    let connecter = port();
    rustrt::aio_connect(ip_to_sbuf(ip), portnum, chan(connecter));
    let client = recv(connecter);
    new_client(client, evt);
}

fn new_client(client: client, evt: chan<socket_event>) {
    // Start the read before notifying about the connect.  This avoids a race
    // condition where the receiver can close the socket before we start
    // reading.
    let reader: port<[u8]> = port();
    rustrt::aio_read(client, chan(reader));

    send(evt, connected(client));

    while true {
        log "waiting for bytes";
        let data: [u8] = recv(reader);
        log "got some bytes";
        log vec::len::<u8>(data);
        if vec::len::<u8>(data) == 0u {
            log "got empty buffer, bailing";
            break;
        }
        log "got non-empty buffer, sending";
        send(evt, received(data));
        log "sent non-empty buffer";
    }
    log "done reading";
    send(evt, closed);
    log "close message sent";
}

fn# accept_task(args: (client, chan<server_event>)) {
    let (client, events) = args;
    log "accept task was spawned";
    let p = port();
    send(events, pending(chan(p)));
    let evt = recv(p);
    new_client(client, evt);
    log "done accepting";
}

fn# server_task(args: (net::ip_addr, int, chan<server_event>,
                       chan<server>)) {
    let (ip, portnum, events, server) = args;
    let accepter = port();
    send(server, rustrt::aio_serve(ip_to_sbuf(ip), portnum, chan(accepter)));

    let client: client;
    while true {
        log "preparing to accept a client";
        client = recv(accepter);
        if rustrt::aio_is_null_client(client) {
            log "client was actually null, returning";
            ret;
        } else { task::spawn2((client, events), accept_task); }
    }
}

fn# request_task(c: chan<ctx>) {
    // Create a port to accept IO requests on
    let p = port();
    // Hand of its channel to our spawner
    send(c, chan(p));
    log "uv run task spawned";
    // Spin for requests
    let req: request;
    while true {
        req = recv(p);
        alt req {
          quit. {
            log "got quit message";
            log "stopping libuv";
            rustrt::aio_stop();
            ret;
          }
          connect(remote(ip, portnum), client) {
            task::spawn2((ip, portnum, client), connect_task);
          }
          serve(ip, portnum, events, server) {
            task::spawn2((ip, portnum, events, server), server_task);
          }
          write(socket, v, status) unsafe {
            rustrt::aio_writedata(socket, vec::unsafe::to_ptr::<u8>(v),
                                  vec::len::<u8>(v), status);
          }
          close_server(server, status) {
            log "closing server";
            rustrt::aio_close_server(server, status);
          }
          close_client(client) {
            log "closing client";
            rustrt::aio_close_socket(client);
          }
        }
    }
}

fn# iotask(c: chan<ctx>) {
    log "io task spawned";
    // Initialize before accepting requests
    rustrt::aio_init();

    log "io task init";
    // Spawn our request task
    let reqtask = task::spawn_joinable2(c, request_task);

    log "uv run task init";
    // Enter IO loop. This never returns until aio_stop is called.
    rustrt::aio_run();
    log "waiting for request task to finish";

    task::join(reqtask);
}

fn new() -> ctx {
    let p: port<ctx> = port();
    task::spawn2(chan(p), iotask);
    ret recv(p);
}

// Local Variables:
// mode: rust;
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C .. 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
