#![unstable(feature = "futures_api",
            reason = "futures in libcore are unstable",
            issue = "50547")]

use fmt;
use future::{FutureObj, LocalFutureObj};

/// Spawns tasks that poll futures to completion onto its associated task
/// executor.
///
/// The term "task" refers to a kind of lightweight "thread". Task executors
/// are responsible for scheduling the execution of tasks on operating system
/// threads.
pub trait Spawn {
    /// Spawns a new task with the given future. The future will be polled until
    /// completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks, either because it has
    /// been shut down or is resource-constrained.
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// # Returns
    ///
    /// An `Ok` return means the executor is *likely* (but not guaranteed)
    /// to accept a subsequent spawn attempt. Likewise, an `Err` return
    /// means that `spawn` is likely, but not guaranteed, to yield an error.
    #[inline]
    fn status(&self) -> Result<(), SpawnErrorKind> {
        Ok(())
    }
}

/// Provides the reason that an executor was unable to spawn.
pub struct SpawnErrorKind {
    _hidden: (),
}

impl fmt::Debug for SpawnErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("SpawnErrorKind")
            .field(&"shutdown")
            .finish()
    }
}

impl SpawnErrorKind {
    /// Spawning is failing because the executor has been shut down.
    pub fn shutdown() -> SpawnErrorKind {
        SpawnErrorKind { _hidden: () }
    }

    /// Check whether this error is the `shutdown` error.
    pub fn is_shutdown(&self) -> bool {
        true
    }
}

/// The result of a failed spawn
#[derive(Debug)]
pub struct SpawnObjError {
    /// The kind of error
    pub kind: SpawnErrorKind,

    /// The future for which spawning inside a task was attempted
    pub future: FutureObj<'static, ()>,
}

/// The result of a failed spawn
#[derive(Debug)]
pub struct SpawnLocalObjError {
    /// The kind of error
    pub kind: SpawnErrorKind,

    /// The future for which spawning inside a task was attempted
    pub future: LocalFutureObj<'static, ()>,
}
