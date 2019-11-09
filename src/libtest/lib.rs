//! Support code for rustc's built in unit-test and micro-benchmarking
//! framework.
//!
//! Almost all user code will only be interested in `Bencher` and
//! `black_box`. All other interactions (such as writing tests and
//! benchmarks themselves) should be done via the `#[test]` and
//! `#[bench]` attributes.
//!
//! See the [Testing Chapter](../book/ch11-00-testing.html) of the book for more details.

// Currently, not much of this is meant for users. It is intended to
// support the simplest interface possible for representing and
// running tests while providing a base that other test frameworks may
// build off of.

// N.B., this is also specified in this crate's Cargo.toml, but libsyntax contains logic specific to
// this crate, which relies on this attribute (rather than the value of `--crate-name` passed by
// cargo) to detect this crate.

#![crate_name = "test"]
#![unstable(feature = "test", issue = "50297")]
#![doc(html_root_url = "https://doc.rust-lang.org/nightly/", test(attr(deny(warnings))))]
#![feature(asm)]
#![cfg_attr(any(unix, target_os = "cloudabi"), feature(libc))]
#![feature(rustc_private)]
#![feature(nll)]
#![feature(set_stdio)]
#![feature(panic_unwind)]
#![feature(staged_api)]
#![feature(termination_trait_lib)]
#![feature(test)]

// Public reexports
pub use self::ColorConfig::*;
pub use self::types::*;
pub use self::types::TestName::*;
pub use self::options::{ColorConfig, Options, OutputFormat, RunIgnored, ShouldPanic};
pub use self::bench::{Bencher, black_box};
pub use self::console::run_tests_console;
pub use cli::TestOpts;

// Module to be used by rustc to compile tests in libtest
pub mod test {
    pub use crate::{
        bench::Bencher,
        cli::{parse_opts, TestOpts},
        helpers::metrics::{Metric, MetricMap},
        options::{ShouldPanic, Options, RunIgnored, RunStrategy},
        test_result::{TestResult, TrFailed, TrFailedMsg, TrIgnored, TrOk},
        time::{TestTimeOptions, TestExecTime},
        types::{
            DynTestFn, DynTestName, StaticBenchFn, StaticTestFn, StaticTestName,
            TestDesc, TestDescAndFn, TestName, TestType,
        },
        assert_test_result, filter_tests, run_test, test_main, test_main_static,
    };
}

use std::{
    env,
    io,
    io::prelude::Write,
    panic::{self, catch_unwind, AssertUnwindSafe, PanicInfo},
    process,
    process::{Command, Termination},
    sync::mpsc::{channel, Sender},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub mod stats;
pub mod bench;
mod formatters;
mod cli;
mod console;
mod event;
mod helpers;
mod time;
mod types;
mod options;
mod test_result;

#[cfg(test)]
mod tests;

use test_result::*;
use time::TestExecTime;
use options::{RunStrategy, Concurrent};
use event::{CompletedTest, TestEvent};
use helpers::sink::Sink;
use helpers::concurrency::get_concurrency;
use helpers::exit_code::get_exit_code;

// Process exit code to be used to indicate test failures.
const ERROR_EXIT_CODE: i32 = 101;

const SECONDARY_TEST_INVOKER_VAR: &'static str = "__RUST_TEST_INVOKE";

// The default console test runner. It accepts the command line
// arguments and a vector of test_descs.
pub fn test_main(args: &[String], tests: Vec<TestDescAndFn>, options: Option<Options>) {
    let mut opts = match cli::parse_opts(args) {
        Some(Ok(o)) => o,
        Some(Err(msg)) => {
            eprintln!("error: {}", msg);
            process::exit(ERROR_EXIT_CODE);
        }
        None => return,
    };
    if let Some(options) = options {
        opts.options = options;
    }
    if opts.list {
        if let Err(e) = console::list_tests_console(&opts, tests) {
            eprintln!("error: io error when listing tests: {:?}", e);
            process::exit(ERROR_EXIT_CODE);
        }
    } else {
        match console::run_tests_console(&opts, tests) {
            Ok(true) => {}
            Ok(false) => process::exit(ERROR_EXIT_CODE),
            Err(e) => {
                eprintln!("error: io error when listing tests: {:?}", e);
                process::exit(ERROR_EXIT_CODE);
            }
        }
    }
}

/// A variant optimized for invocation with a static test vector.
/// This will panic (intentionally) when fed any dynamic tests.
///
/// This is the entry point for the main function generated by `rustc --test`
/// when panic=unwind.
pub fn test_main_static(tests: &[&TestDescAndFn]) {
    let args = env::args().collect::<Vec<_>>();
    let owned_tests: Vec<_> = tests.iter().map(make_owned_test).collect();
    test_main(&args, owned_tests, None)
}

/// A variant optimized for invocation with a static test vector.
/// This will panic (intentionally) when fed any dynamic tests.
///
/// Runs tests in panic=abort mode, which involves spawning subprocesses for
/// tests.
///
/// This is the entry point for the main function generated by `rustc --test`
/// when panic=abort.
pub fn test_main_static_abort(tests: &[&TestDescAndFn]) {
    // If we're being run in SpawnedSecondary mode, run the test here. run_test
    // will then exit the process.
    if let Ok(name) = env::var(SECONDARY_TEST_INVOKER_VAR) {
        let test = tests
            .iter()
            .filter(|test| test.desc.name.as_slice() == name)
            .map(make_owned_test)
            .next()
            .expect("couldn't find a test with the provided name");
        let TestDescAndFn { desc, testfn } = test;
        let testfn = match testfn {
            StaticTestFn(f) => f,
            _ => panic!("only static tests are supported"),
        };
        run_test_in_spawned_subprocess(desc, Box::new(testfn));
    }

    let args = env::args().collect::<Vec<_>>();
    let owned_tests: Vec<_> = tests.iter().map(make_owned_test).collect();
    test_main(&args, owned_tests, Some(Options::new().panic_abort(true)))
}

/// Clones static values for putting into a dynamic vector, which test_main()
/// needs to hand out ownership of tests to parallel test runners.
///
/// This will panic when fed any dynamic tests, because they cannot be cloned.
fn make_owned_test(test: &&TestDescAndFn) -> TestDescAndFn {
    match test.testfn {
        StaticTestFn(f) => TestDescAndFn {
            testfn: StaticTestFn(f),
            desc: test.desc.clone(),
        },
        StaticBenchFn(f) => TestDescAndFn {
            testfn: StaticBenchFn(f),
            desc: test.desc.clone(),
        },
        _ => panic!("non-static tests passed to test::test_main_static"),
    }
}

/// Invoked when unit tests terminate. Should panic if the unit
/// Tests is considered a failure. By default, invokes `report()`
/// and checks for a `0` result.
pub fn assert_test_result<T: Termination>(result: T) {
    let code = result.report();
    assert_eq!(
        code, 0,
        "the test returned a termination value with a non-zero status code ({}) \
         which indicates a failure",
        code
    );
}

pub fn run_tests<F>(
    opts: &TestOpts,
    tests: Vec<TestDescAndFn>,
    mut notify_about_test_event: F
) -> io::Result<()>
where
    F: FnMut(TestEvent) -> io::Result<()>,
{
    use std::collections::{self, HashMap};
    use std::hash::BuildHasherDefault;
    use std::sync::mpsc::RecvTimeoutError;
    // Use a deterministic hasher
    type TestMap =
        HashMap<TestDesc, Instant, BuildHasherDefault<collections::hash_map::DefaultHasher>>;

    let tests_len = tests.len();

    let mut filtered_tests = filter_tests(opts, tests);
    if !opts.bench_benchmarks {
        filtered_tests = convert_benchmarks_to_tests(filtered_tests);
    }

    let filtered_tests = {
        let mut filtered_tests = filtered_tests;
        for test in filtered_tests.iter_mut() {
            test.desc.name = test.desc.name.with_padding(test.testfn.padding());
        }

        filtered_tests
    };

    let filtered_out = tests_len - filtered_tests.len();
    let event = TestEvent::TeFilteredOut(filtered_out);
    notify_about_test_event(event)?;

    let filtered_descs = filtered_tests.iter().map(|t| t.desc.clone()).collect();

    let event = TestEvent::TeFiltered(filtered_descs);
    notify_about_test_event(event)?;

    let (filtered_tests, filtered_benchs): (Vec<_>, _) =
        filtered_tests.into_iter().partition(|e| match e.testfn {
            StaticTestFn(_) | DynTestFn(_) => true,
            _ => false,
        });

    let concurrency = opts.test_threads.unwrap_or_else(get_concurrency);

    let mut remaining = filtered_tests;
    remaining.reverse();
    let mut pending = 0;

    let (tx, rx) = channel::<CompletedTest>();
    let run_strategy = if opts.options.panic_abort {
        RunStrategy::SpawnPrimary
    } else {
        RunStrategy::InProcess
    };

    let mut running_tests: TestMap = HashMap::default();

    fn get_timed_out_tests(running_tests: &mut TestMap) -> Vec<TestDesc> {
        let now = Instant::now();
        let timed_out = running_tests
            .iter()
            .filter_map(|(desc, timeout)| {
                if &now >= timeout {
                    Some(desc.clone())
                } else {
                    None
                }
            })
            .collect();
        for test in &timed_out {
            running_tests.remove(test);
        }
        timed_out
    };

    fn calc_timeout(running_tests: &TestMap) -> Option<Duration> {
        running_tests.values().min().map(|next_timeout| {
            let now = Instant::now();
            if *next_timeout >= now {
                *next_timeout - now
            } else {
                Duration::new(0, 0)
            }
        })
    };

    if concurrency == 1 {
        while !remaining.is_empty() {
            let test = remaining.pop().unwrap();
            let event = TestEvent::TeWait(test.desc.clone());
            notify_about_test_event(event)?;
            run_test(opts, !opts.run_tests, test, run_strategy, tx.clone(), Concurrent::No);
            let completed_test = rx.recv().unwrap();

            let event = TestEvent::TeResult(completed_test);
            notify_about_test_event(event)?;
        }
    } else {
        while pending > 0 || !remaining.is_empty() {
            while pending < concurrency && !remaining.is_empty() {
                let test = remaining.pop().unwrap();
                let timeout = time::get_default_test_timeout();
                running_tests.insert(test.desc.clone(), timeout);

                let event = TestEvent::TeWait(test.desc.clone());
                notify_about_test_event(event)?; //here no pad
                run_test(opts, !opts.run_tests, test, run_strategy, tx.clone(), Concurrent::Yes);
                pending += 1;
            }

            let mut res;
            loop {
                if let Some(timeout) = calc_timeout(&running_tests) {
                    res = rx.recv_timeout(timeout);
                    for test in get_timed_out_tests(&mut running_tests) {
                        let event = TestEvent::TeTimeout(test);
                        notify_about_test_event(event)?;
                    }

                    match res {
                        Err(RecvTimeoutError::Timeout) => {
                            // Result is not yet ready, continue waiting.
                        }
                        _ => {
                            // We've got a result, stop the loop.
                            break;
                        }
                    }
                } else {
                    res = rx.recv().map_err(|_| RecvTimeoutError::Disconnected);
                    break;
                }
            }

            let completed_test = res.unwrap();
            running_tests.remove(&completed_test.desc);

            let event = TestEvent::TeResult(completed_test);
            notify_about_test_event(event)?;
            pending -= 1;
        }
    }

    if opts.bench_benchmarks {
        // All benchmarks run at the end, in serial.
        for b in filtered_benchs {
            let event = TestEvent::TeWait(b.desc.clone());
            notify_about_test_event(event)?;
            run_test(opts, false, b, run_strategy, tx.clone(), Concurrent::No);
            let completed_test = rx.recv().unwrap();

            let event = TestEvent::TeResult(completed_test);
            notify_about_test_event(event)?;
        }
    }
    Ok(())
}

pub fn filter_tests(opts: &TestOpts, tests: Vec<TestDescAndFn>) -> Vec<TestDescAndFn> {
    let mut filtered = tests;
    let matches_filter = |test: &TestDescAndFn, filter: &str| {
        let test_name = test.desc.name.as_slice();

        match opts.filter_exact {
            true => test_name == filter,
            false => test_name.contains(filter),
        }
    };

    // Remove tests that don't match the test filter
    if let Some(ref filter) = opts.filter {
        filtered.retain(|test| matches_filter(test, filter));
    }

    // Skip tests that match any of the skip filters
    filtered.retain(|test| !opts.skip.iter().any(|sf| matches_filter(test, sf)));

    // Excludes #[should_panic] tests
    if opts.exclude_should_panic {
        filtered.retain(|test| test.desc.should_panic == ShouldPanic::No);
    }

    // maybe unignore tests
    match opts.run_ignored {
        RunIgnored::Yes => {
            filtered
                .iter_mut()
                .for_each(|test| test.desc.ignore = false);
        }
        RunIgnored::Only => {
            filtered.retain(|test| test.desc.ignore);
            filtered
                .iter_mut()
                .for_each(|test| test.desc.ignore = false);
        }
        RunIgnored::No => {}
    }

    // Sort the tests alphabetically
    filtered.sort_by(|t1, t2| t1.desc.name.as_slice().cmp(t2.desc.name.as_slice()));

    filtered
}

pub fn convert_benchmarks_to_tests(tests: Vec<TestDescAndFn>) -> Vec<TestDescAndFn> {
    // convert benchmarks to tests, if we're not benchmarking them
    tests
        .into_iter()
        .map(|x| {
            let testfn = match x.testfn {
                DynBenchFn(bench) => DynTestFn(Box::new(move || {
                    bench::run_once(|b| __rust_begin_short_backtrace(|| bench.run(b)))
                })),
                StaticBenchFn(benchfn) => DynTestFn(Box::new(move || {
                    bench::run_once(|b| __rust_begin_short_backtrace(|| benchfn(b)))
                })),
                f => f,
            };
            TestDescAndFn {
                desc: x.desc,
                testfn,
            }
        })
        .collect()
}

pub fn run_test(
    opts: &TestOpts,
    force_ignore: bool,
    test: TestDescAndFn,
    strategy: RunStrategy,
    monitor_ch: Sender<CompletedTest>,
    concurrency: Concurrent,
) {
    let TestDescAndFn { desc, testfn } = test;

    // Emscripten can catch panics but other wasm targets cannot
    let ignore_because_no_process_support = desc.should_panic != ShouldPanic::No
        && cfg!(target_arch = "wasm32") && !cfg!(target_os = "emscripten");

    if force_ignore || desc.ignore || ignore_because_no_process_support {
        let message = CompletedTest::new(desc, TrIgnored, None, Vec::new());
        monitor_ch.send(message).unwrap();
        return;
    }

    struct TestRunOpts {
        pub strategy: RunStrategy,
        pub nocapture: bool,
        pub concurrency: Concurrent,
        pub time: Option<time::TestTimeOptions>,
    }

    fn run_test_inner(
        desc: TestDesc,
        monitor_ch: Sender<CompletedTest>,
        testfn: Box<dyn FnOnce() + Send>,
        opts: TestRunOpts,
    ) {
        let concurrency = opts.concurrency;
        let name = desc.name.clone();

        let runtest = move || {
            match opts.strategy {
                RunStrategy::InProcess =>
                    run_test_in_process(
                        desc,
                        opts.nocapture,
                        opts.time.is_some(),
                        testfn,
                        monitor_ch,
                        opts.time
                    ),
                RunStrategy::SpawnPrimary =>
                    spawn_test_subprocess(desc, opts.time.is_some(), monitor_ch, opts.time),
            }
        };

        // If the platform is single-threaded we're just going to run
        // the test synchronously, regardless of the concurrency
        // level.
        let supports_threads = !cfg!(target_os = "emscripten") && !cfg!(target_arch = "wasm32");
        if concurrency == Concurrent::Yes && supports_threads {
            let cfg = thread::Builder::new().name(name.as_slice().to_owned());
            cfg.spawn(runtest).unwrap();
        } else {
            runtest();
        }
    }

    let test_run_opts = TestRunOpts {
        strategy,
        nocapture: opts.nocapture,
        concurrency,
        time: opts.time_options
    };

    match testfn {
        DynBenchFn(bencher) => {
            // Benchmarks aren't expected to panic, so we run them all in-process.
            crate::bench::benchmark(desc, monitor_ch, opts.nocapture, |harness| {
                bencher.run(harness)
            });
        }
        StaticBenchFn(benchfn) => {
            // Benchmarks aren't expected to panic, so we run them all in-process.
            crate::bench::benchmark(desc, monitor_ch, opts.nocapture, |harness| {
                (benchfn.clone())(harness)
            });
        }
        DynTestFn(f) => {
            match strategy {
                RunStrategy::InProcess => (),
                _ => panic!("Cannot run dynamic test fn out-of-process"),
            };
            run_test_inner(
                desc,
                monitor_ch,
                Box::new(move || __rust_begin_short_backtrace(f)),
                test_run_opts,
            );
        }
        StaticTestFn(f) => run_test_inner(
            desc,
            monitor_ch,
            Box::new(move || __rust_begin_short_backtrace(f)),
            test_run_opts,
        ),
    }
}

/// Fixed frame used to clean the backtrace with `RUST_BACKTRACE=1`.
#[inline(never)]
fn __rust_begin_short_backtrace<F: FnOnce()>(f: F) {
    f()
}

fn run_test_in_process(
    desc: TestDesc,
    nocapture: bool,
    report_time: bool,
    testfn: Box<dyn FnOnce() + Send>,
    monitor_ch: Sender<CompletedTest>,
    time_opts: Option<time::TestTimeOptions>,
) {
    // Buffer for capturing standard I/O
    let data = Arc::new(Mutex::new(Vec::new()));

    let oldio = if !nocapture {
        Some((
            io::set_print(Some(Sink::new_boxed(&data))),
            io::set_panic(Some(Sink::new_boxed(&data))),
        ))
    } else {
        None
    };

    let start = if report_time {
        Some(Instant::now())
    } else {
        None
    };
    let result = catch_unwind(AssertUnwindSafe(testfn));
    let exec_time = start.map(|start| {
        let duration = start.elapsed();
        TestExecTime(duration)
    });

    if let Some((printio, panicio)) = oldio {
        io::set_print(printio);
        io::set_panic(panicio);
    }

    let test_result = match result {
        Ok(()) => calc_result(&desc, Ok(()), &time_opts, &exec_time),
        Err(e) => calc_result(&desc, Err(e.as_ref()), &time_opts, &exec_time),
    };
    let stdout = data.lock().unwrap().to_vec();
    let message = CompletedTest::new(desc.clone(), test_result, exec_time, stdout);
    monitor_ch.send(message).unwrap();
}

fn spawn_test_subprocess(
    desc: TestDesc,
    report_time: bool,
    monitor_ch: Sender<CompletedTest>,
    time_opts: Option<time::TestTimeOptions>,
) {
    let (result, test_output, exec_time) = (|| {
        let args = env::args().collect::<Vec<_>>();
        let current_exe = &args[0];

        let start = if report_time {
            Some(Instant::now())
        } else {
            None
        };
        let output = match Command::new(current_exe)
            .env(SECONDARY_TEST_INVOKER_VAR, desc.name.as_slice())
            .output() {
                Ok(out) => out,
                Err(e) => {
                    let err = format!("Failed to spawn {} as child for test: {:?}", args[0], e);
                    return (TrFailed, err.into_bytes(), None);
                }
            };
        let exec_time = start.map(|start| {
            let duration = start.elapsed();
            TestExecTime(duration)
        });

        let std::process::Output { stdout, stderr, status } = output;
        let mut test_output = stdout;
        formatters::write_stderr_delimiter(&mut test_output, &desc.name);
        test_output.extend_from_slice(&stderr);

        let result = match (|| -> Result<TestResult, String> {
            let exit_code = get_exit_code(status)?;
            Ok(get_result_from_exit_code(&desc, exit_code, &time_opts, &exec_time))
        })() {
            Ok(r) => r,
            Err(e) => {
                write!(&mut test_output, "Unexpected error: {}", e).unwrap();
                TrFailed
            }
        };

        (result, test_output, exec_time)
    })();

    let message = CompletedTest::new(desc.clone(), result, exec_time, test_output);
    monitor_ch.send(message).unwrap();
}

fn run_test_in_spawned_subprocess(
    desc: TestDesc,
    testfn: Box<dyn FnOnce() + Send>,
) -> ! {
    let builtin_panic_hook = panic::take_hook();
    let record_result = Arc::new(move |panic_info: Option<&'_ PanicInfo<'_>>| {
        let test_result = match panic_info {
            Some(info) => calc_result(&desc, Err(info.payload()), &None, &None),
            None => calc_result(&desc, Ok(()), &None, &None),
        };

        // We don't support serializing TrFailedMsg, so just
        // print the message out to stderr.
        if let TrFailedMsg(msg) = &test_result {
            eprintln!("{}", msg);
        }

        if let Some(info) = panic_info {
            builtin_panic_hook(info);
        }

        if let TrOk = test_result {
            process::exit(test_result::TR_OK);
        } else {
            process::exit(test_result::TR_FAILED);
        }
    });
    let record_result2 = record_result.clone();
    panic::set_hook(Box::new(move |info| record_result2(Some(&info))));
    testfn();
    record_result(None);
    unreachable!("panic=abort callback should have exited the process")
}
