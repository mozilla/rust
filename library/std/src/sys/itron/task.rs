use super::{abi, error::ItronError};

use crate::mem::MaybeUninit;

/// Get the ID of the task in Running state.
#[inline]
pub fn current_task_id() -> Result<abi::ID, ItronError> {
    unsafe {
        let mut out = MaybeUninit::uninit();
        ItronError::err_if_negative(abi::get_tid(out.as_mut_ptr()))?;
        Ok(out.assume_init())
    }
}

/// Get the specified task's priority.
#[inline]
pub fn task_priority(task: abi::ID) -> Result<abi::PRI, ItronError> {
    unsafe {
        let mut out = MaybeUninit::uninit();
        ItronError::err_if_negative(abi::get_pri(task, out.as_mut_ptr()))?;
        Ok(out.assume_init())
    }
}
