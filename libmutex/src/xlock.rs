use crate::deadline::Deadline;
use crate::utils;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

unsafe impl<T: ?Sized + Send, S: Spec> Send for XLock<T, S> {}
unsafe impl<T: ?Sized + Send + Sync, S: Spec> Sync for XLock<T, S> {}
unsafe impl<T: ?Sized + Sync, S: Spec> Sync for LockReadGuard<'_, T, S> {}
unsafe impl<T: ?Sized + Sync, S: Spec> Sync for LockWriteGuard<'_, T, S> {}

pub trait Spec: Debug {
    type Sync;

    fn new() -> Self::Sync;

    fn try_read(sync: &Self::Sync, duration: Duration) -> bool;

    fn read_unlock(sync: &Self::Sync);

    fn try_write(sync: &Self::Sync, duration: Duration) -> bool;

    fn write_unlock(sync: &Self::Sync);

    fn downgrade(sync: &Self::Sync);

    fn try_upgrade(sync: &Self::Sync, duration: Duration) -> bool;
}

#[derive(Debug)]
pub struct ReadBiased;

#[derive(Debug)]
pub struct ReadBiasedSync {
    state: Mutex<ReadBiasedState>,
    cond: Condvar
}

#[derive(Debug)]
struct ReadBiasedState {
    readers: u32,
    writer: bool,
}

impl Spec for ReadBiased {
    type Sync = ReadBiasedSync;

    #[inline]
    fn new() -> Self::Sync {
        Self::Sync {
            state: Mutex::new(ReadBiasedState { readers: 0, writer: false }),
            cond: Condvar::new()
        }
    }

    #[inline]
    fn try_read(sync: &Self::Sync, duration: Duration) -> bool {
        let mut deadline = Deadline::lazy_after(duration);
        let mut state = utils::remedy(sync.state.lock());
        while state.writer {
            let (guard, timed_out) =
                utils::cond_wait_remedy(&sync.cond, state, deadline.remaining());

            if timed_out {
                return false
            }
            state = guard;
        }
        state.readers += 1;
        true
    }

    #[inline]
    fn read_unlock(sync: &Self::Sync) {
        let mut state = utils::remedy(sync.state.lock());
        debug_assert!(state.readers > 0, "readers: {}", state.readers);
        debug_assert!(!state.writer);
        state.readers -= 1;
        let readers = state.readers;
        drop(state);
        if readers == 1 {
            sync.cond.notify_all();
        } else if readers == 0 {
            sync.cond.notify_one()
        }
    }

    #[inline]
    fn try_write(sync: &Self::Sync, duration: Duration) -> bool {
        let mut deadline = Deadline::lazy_after(duration);
        let mut state = utils::remedy(sync.state.lock());
        while state.readers != 0 || state.writer {
            let (guard, timed_out) =
                utils::cond_wait_remedy(&sync.cond, state, deadline.remaining());

            if timed_out {
                return false;
            }
            state = guard;
        }
        state.writer = true;
        true
    }

    #[inline]
    fn write_unlock(sync: &Self::Sync) {
        let mut state = utils::remedy(sync.state.lock());
        debug_assert!(state.readers == 0, "readers: {}", state.readers);
        debug_assert!(state.writer);
        state.writer = false;
        drop(state);
        sync.cond.notify_one();
    }

    fn downgrade(sync: &Self::Sync) {
        let mut state = utils::remedy(sync.state.lock());
        debug_assert!(state.readers == 0, "readers: {}", state.readers);
        debug_assert!(state.writer);
        state.readers = 1;
        state.writer = false;
        drop(state);
        sync.cond.notify_all();
    }

    fn try_upgrade(sync: &Self::Sync, duration: Duration) -> bool {
        let mut deadline = Deadline::lazy_after(duration);
        let mut state = utils::remedy(sync.state.lock());
        debug_assert!(state.readers > 0, "readers: {}", state.readers);
        debug_assert!(!state.writer);
        while state.readers != 1 {
            let (guard, timed_out) =
                utils::cond_wait_remedy(&sync.cond, state, deadline.remaining());

            if timed_out {
                return false
            }
            state = guard;
            debug_assert!(state.readers > 0, "readers: {}", state.readers);
            debug_assert!(!state.writer);
        }
        state.readers = 0;
        state.writer = true;
        true
    }
}

#[derive(Debug)]
pub struct XLock<T: ?Sized, S: Spec> {
    sync: S::Sync,
    data: UnsafeCell<T>,
}

impl<T, S: Spec> XLock<T, S> {
    #[inline]
    pub fn new(t: T) -> Self {
        Self {
            sync: S::new(),
            data: UnsafeCell::new(t),
        }
    }

    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized, S: Spec> XLock<T, S> {
    #[inline]
    pub fn read(&self) -> LockReadGuard<'_, T, S> {
        self.try_read(Duration::MAX).unwrap()
    }

    #[inline]
    pub fn try_read(&self, duration: Duration) -> Option<LockReadGuard<'_, T, S>> {
        if S::try_read(&self.sync, duration) {
            let data = unsafe { NonNull::new_unchecked(self.data.get()) };
            Some(LockReadGuard {
                data,
                lock: self,
                locked: true,
                __no_send: PhantomData::default(),
            })
        } else {
            None
        }
    }

    #[inline]
    fn read_unlock(&self) {
        S::read_unlock(&self.sync);
    }

    #[inline]
    pub fn write(&self) -> LockWriteGuard<'_, T, S> {
        self.try_write(Duration::MAX).unwrap()
    }

    #[inline]
    pub fn try_write(&self, duration: Duration) -> Option<LockWriteGuard<'_, T, S>> {
        if S::try_write(&self.sync, duration) {
            Some(LockWriteGuard {
                lock: self,
                locked: true,
                __no_send: PhantomData::default(),
            })
        } else {
            None
        }
    }

    #[inline]
    fn write_unlock(&self) {
        S::write_unlock(&self.sync);
    }

    #[inline]
    pub fn downgrade(&self) -> LockReadGuard<T, S> {
        S::downgrade(&self.sync);
        let data = unsafe { NonNull::new_unchecked(self.data.get()) };
        LockReadGuard {
            data,
            lock: self,
            locked: true,
            __no_send: PhantomData::default(),
        }
    }

    #[inline]
    fn upgrade(&self) -> LockWriteGuard<'_, T, S> {
        self.try_upgrade(Duration::MAX).unwrap()
    }

    #[inline]
    fn try_upgrade(&self, duration: Duration) -> Option<LockWriteGuard<'_, T, S>> {
        if S::try_upgrade(&self.sync, duration) {
            Some(LockWriteGuard {
                lock: self,
                locked: true,
                __no_send: PhantomData::default(),
            })
        } else {
            None
        }
    }
}

pub struct LockReadGuard<'a, T: ?Sized, S: Spec> {
    data: NonNull<T>,
    lock: &'a XLock<T, S>,
    locked: bool,

    /// Emulates !Send for the struct. (Until issue 68318 -- negative trait bounds -- is resolved.)
    __no_send: PhantomData<*const ()>,
}

impl<T: ?Sized, S: Spec> Drop for LockReadGuard<'_, T, S> {
    #[inline]
    fn drop(&mut self) {
        if self.locked {
            self.lock.read_unlock();
        }
    }
}

impl<'a, T: ?Sized, S: Spec> LockReadGuard<'a, T, S> {
    #[inline]
    pub fn upgrade(mut self) -> LockWriteGuard<'a, T, S> {
        self.locked = false;
        self.lock.upgrade()
    }

    #[inline]
    pub fn try_upgrade(mut self, duration: Duration) -> UpgradeOutcome<'a, T, S> {
        match self.lock.try_upgrade(duration) {
            None => UpgradeOutcome::Unchanged(self),
            Some(guard) => {
                self.locked = false;
                UpgradeOutcome::Upgraded(guard)
            }
        }
    }
}

impl<T: ?Sized, S: Spec> Deref for LockReadGuard<'_, T, S> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }
}

pub struct LockWriteGuard<'a, T: ?Sized, S: Spec> {
    lock: &'a XLock<T, S>,
    locked: bool,
    /// Emulates !Send for the struct. (Until issue 68318 -- negative trait bounds -- is resolved.)
    __no_send: PhantomData<*const ()>,
}

impl<T: ?Sized, S: Spec> Drop for LockWriteGuard<'_, T, S> {
    #[inline]
    fn drop(&mut self) {
        if self.locked {
            self.lock.write_unlock();
        }
    }
}

impl<'a, T: ?Sized, S: Spec> LockWriteGuard<'a, T, S> {
    #[inline]
    pub fn downgrade(mut self) -> LockReadGuard<'a, T, S> {
        self.locked = false;
        self.lock.downgrade()
    }
}

impl<T: ?Sized, S: Spec> Deref for LockWriteGuard<'_, T, S> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized, S: Spec> DerefMut for LockWriteGuard<'_, T, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

pub enum UpgradeOutcome<'a, T: ?Sized, S: Spec> {
    Upgraded(LockWriteGuard<'a, T, S>),
    Unchanged(LockReadGuard<'a, T, S>),
}

impl<'a, T: ?Sized, S: Spec> UpgradeOutcome<'a, T, S> {
    #[inline]
    pub fn is_upgraded(&self) -> bool {
        matches!(self, UpgradeOutcome::Upgraded(_))
    }

    #[inline]
    pub fn is_unchanged(&self) -> bool {
        matches!(self, UpgradeOutcome::Unchanged(_))
    }

    #[inline]
    pub fn upgraded(self) -> Option<LockWriteGuard<'a, T, S>> {
        match self {
            UpgradeOutcome::Upgraded(guard) => Some(guard),
            UpgradeOutcome::Unchanged(_) => None,
        }
    }

    #[inline]
    pub fn unchanged(self) -> Option<LockReadGuard<'a, T, S>> {
        match self {
            UpgradeOutcome::Upgraded(_) => None,
            UpgradeOutcome::Unchanged(guard) => Some(guard),
        }
    }
}

#[cfg(test)]
mod tests;