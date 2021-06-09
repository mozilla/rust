//! A readers-writer lock implementation backed by the SOLID kernel extension.
use super::{
    abi,
    itron::{error::ItronError, spin::SpinIdOnceCell},
};

pub struct RWLock {
    /// The ID of the underlying mutex object
    rwl: SpinIdOnceCell<()>,
}

// Safety: `num_readers` is protected by `mtx_num_readers`
unsafe impl Send for RWLock {}
unsafe impl Sync for RWLock {}

fn new_rwl() -> abi::ID {
    ItronError::err_if_negative(unsafe { abi::rwl_acre_rwl() }).expect("acre_mtx failed")
}

impl RWLock {
    pub const fn new() -> RWLock {
        RWLock { rwl: SpinIdOnceCell::new() }
    }

    /// Get the inner mutex's ID, which is lazily created.
    fn raw(&self) -> abi::ID {
        let Ok((id, _)) = self.rwl.get_or_try_init(|| Ok::<_, !>((new_rwl(), ())));
        id
    }

    #[inline]
    pub unsafe fn read(&self) {
        let rwl = self.raw();
        ItronError::err_if_negative(unsafe { abi::rwl_loc_rdl(rwl) }).expect("rwl_loc_rdl failed");
    }

    #[inline]
    pub unsafe fn try_read(&self) -> bool {
        let rwl = self.raw();
        ItronError::err_if_negative(unsafe { abi::rwl_ploc_rdl(rwl) })
            .map(|_| true)
            .or_else(|e| if e.as_raw() != abi::E_TMOUT { Err(e) } else { Ok(false) })
            .expect("rwl_ploc_rdl failed")
    }

    #[inline]
    pub unsafe fn write(&self) {
        let rwl = self.raw();
        ItronError::err_if_negative(unsafe { abi::rwl_loc_wrl(rwl) }).expect("rwl_loc_wrl failed");
    }

    #[inline]
    pub unsafe fn try_write(&self) -> bool {
        let rwl = self.raw();
        ItronError::err_if_negative(unsafe { abi::rwl_ploc_wrl(rwl) })
            .map(|_| true)
            .or_else(|e| if e.as_raw() != abi::E_TMOUT { Err(e) } else { Ok(false) })
            .expect("rwl_ploc_wrl failed")
    }

    #[inline]
    pub unsafe fn read_unlock(&self) {
        let rwl = self.raw();
        ItronError::err_if_negative(unsafe { abi::rwl_unl_rwl(rwl) }).expect("rwl_unl_rwl failed");
    }

    #[inline]
    pub unsafe fn write_unlock(&self) {
        let rwl = self.raw();
        ItronError::err_if_negative(unsafe { abi::rwl_unl_rwl(rwl) }).expect("rwl_unl_rwl failed");
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        if let Some(rwl) = self.rwl.get().map(|x| x.0) {
            ItronError::err_if_negative(unsafe { abi::rwl_del_rwl(rwl) })
                .expect("rwl_del_rwl failed");
        }
    }
}
