use crate::xlock::{
    ArrivalOrdered, LockReadGuard, LockWriteGuard, Moderator, ReadBiased, UpgradeOutcome,
    WriteBiased, XLock,
};
use std::ops::{Deref, DerefMut};
use std::time::Duration;

pub type LockBox<T> =
    Box<dyn for<'a> Locklike<'a, T, R = DynLockReadGuard<'a, T>, W = DynLockWriteGuard<'a, T>>>;

pub type LockBoxSized<T> = Box<
    dyn for<'a> LocklikeSized<'a, T, R = DynLockReadGuard<'a, T>, W = DynLockWriteGuard<'a, T>>,
>;

// trait Reduce<T: ?Sized> {
//     fn downcast<'a>(self: &'a Box<Self>) -> &'a LockBox<T>;
// }
//
// impl<T: ?Sized + 'static> Reduce<T> for LockBoxSized<T> {
//     fn downcast<'a>(self: &'a Box<Self>) -> &'a LockBox<T> {
//         let a: &'a dyn Any = self;
//         a.downcast_ref::<LockBox<T>>().unwrap()
//     }
// }

pub trait LockReadGuardlike<'a, T: ?Sized>: Deref<Target = T> {
    fn upgrade(self) -> DynLockWriteGuard<'a, T>;

    fn try_upgrade(
        self,
        duration: Duration,
    ) -> UpgradeOutcome<DynLockWriteGuard<'a, T>, DynLockReadGuard<'a, T>>;
}

pub trait LockWriteGuardlike<'a, T: ?Sized>: DerefMut<Target = T> {
    fn downgrade(self) -> DynLockReadGuard<'a, T>;
}

trait LockReadGuardSurrogate<'a, T: ?Sized>: Deref<Target = T> {
    fn upgrade_box(self: Box<Self>) -> DynLockWriteGuard<'a, T>;

    fn try_upgrade_box(
        self: Box<Self>,
        duration: Duration,
    ) -> UpgradeOutcome<DynLockWriteGuard<'a, T>, DynLockReadGuard<'a, T>>;
}

trait LockWriteGuardSurrogate<'a, T: ?Sized>: DerefMut<Target = T> {
    fn downgrade_box(self: Box<Self>) -> DynLockReadGuard<'a, T>;
}

pub trait Locklike<'a, T: ?Sized>: Sync + Send {
    type R: LockReadGuardlike<'a, T>;
    type W: LockWriteGuardlike<'a, T>;

    fn read(&'a self) -> Self::R;

    fn try_read(&'a self, duration: Duration) -> Option<Self::R>;

    fn write(&'a self) -> Self::W;

    fn try_write(&'a self, duration: Duration) -> Option<Self::W>;

    fn get_mut(&mut self) -> &mut T;
}

pub trait LocklikeSized<'a, T>: Locklike<'a, T> {
    fn into_inner(self: Box<Self>) -> T;
}

impl<'a, T: ?Sized + Sync + Send + 'a, M: Moderator + 'a> Locklike<'a, T> for XLock<T, M> {
    type R = LockReadGuard<'a, T, M>;
    type W = LockWriteGuard<'a, T, M>;

    fn read(&'a self) -> Self::R {
        self.read()
    }

    fn try_read(&'a self, duration: Duration) -> Option<Self::R> {
        self.try_read(duration)
    }

    fn write(&'a self) -> Self::W {
        self.write()
    }

    fn try_write(&'a self, duration: Duration) -> Option<Self::W> {
        self.try_write(duration)
    }

    fn get_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

impl<T, M: Moderator> XLock<T, M> {
    fn lock_into_inner(self) -> T {
        self.into_inner()
    }
}

impl<'a, T: Sync + Send + 'a, M: Moderator + 'a> LocklikeSized<'a, T> for XLock<T, M> {
    fn into_inner(self: Box<Self>) -> T {
        self.lock_into_inner()
    }
}

impl<'a, T: ?Sized, M: Moderator> LockReadGuardlike<'a, T> for LockReadGuard<'a, T, M> {
    fn upgrade(self) -> DynLockWriteGuard<'a, T> {
        self.upgrade().into()
    }

    fn try_upgrade(
        self,
        duration: Duration,
    ) -> UpgradeOutcome<DynLockWriteGuard<'a, T>, DynLockReadGuard<'a, T>> {
        self.try_upgrade(duration)
            .map(DynLockWriteGuard::from, DynLockReadGuard::from)
    }
}

impl<'a, T: ?Sized, M: Moderator> LockReadGuardSurrogate<'a, T> for LockReadGuard<'a, T, M> {
    fn upgrade_box(self: Box<Self>) -> DynLockWriteGuard<'a, T> {
        self.upgrade().into()
    }

    fn try_upgrade_box(
        self: Box<Self>,
        duration: Duration,
    ) -> UpgradeOutcome<DynLockWriteGuard<'a, T>, DynLockReadGuard<'a, T>> {
        self.try_upgrade(duration)
            .map(DynLockWriteGuard::from, DynLockReadGuard::from)
    }
}

impl<'a, T: ?Sized, M: Moderator> LockWriteGuardlike<'a, T> for LockWriteGuard<'a, T, M> {
    fn downgrade(self) -> DynLockReadGuard<'a, T> {
        self.downgrade().into()
    }
}

impl<'a, T: ?Sized, M: Moderator> LockWriteGuardSurrogate<'a, T> for LockWriteGuard<'a, T, M> {
    fn downgrade_box(self: Box<Self>) -> DynLockReadGuard<'a, T> {
        self.downgrade().into()
    }
}

struct PolyLock<T: ?Sized, M: Moderator>(XLock<T, M>);

impl<'a, T: ?Sized + Sync + Send + 'a, M: Moderator + 'a> Locklike<'a, T> for PolyLock<T, M> {
    type R = DynLockReadGuard<'a, T>;
    type W = DynLockWriteGuard<'a, T>;

    fn read(&'a self) -> Self::R {
        self.0.read().into()
    }

    fn try_read(&'a self, duration: Duration) -> Option<Self::R> {
        self.0.try_read(duration).map(DynLockReadGuard::from)
    }

    fn write(&'a self) -> Self::W {
        self.0.write().into()
    }

    fn try_write(&'a self, duration: Duration) -> Option<Self::W> {
        self.0.try_write(duration).map(DynLockWriteGuard::from)
    }

    fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }
}

impl<'a, T: Sync + Send + 'a, M: Moderator + 'a> LocklikeSized<'a, T> for PolyLock<T, M> {
    fn into_inner(self: Box<Self>) -> T {
        self.0.into_inner()
    }
}

pub struct DynLockReadGuard<'a, T: ?Sized>(Box<dyn LockReadGuardSurrogate<'a, T> + 'a>);

impl<'a, T: ?Sized> DynLockReadGuard<'a, T> {
    pub fn try_upgrade(
        self,
        duration: Duration,
    ) -> UpgradeOutcome<DynLockWriteGuard<'a, T>, DynLockReadGuard<'a, T>> {
        self.0.try_upgrade_box(duration)
    }
}

impl<T: ?Sized> Deref for DynLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<'a, T: ?Sized> LockReadGuardlike<'a, T> for DynLockReadGuard<'a, T> {
    fn upgrade(self) -> DynLockWriteGuard<'a, T> {
        self.0.upgrade_box().into()
    }

    fn try_upgrade(
        self,
        duration: Duration,
    ) -> UpgradeOutcome<DynLockWriteGuard<'a, T>, DynLockReadGuard<'a, T>> {
        self.try_upgrade(duration)
            .map(DynLockWriteGuard::from, DynLockReadGuard::from)
    }
}

impl<'a, T: ?Sized + 'a, M: Moderator> From<LockReadGuard<'a, T, M>> for DynLockReadGuard<'a, T> {
    fn from(guard: LockReadGuard<'a, T, M>) -> Self {
        DynLockReadGuard(Box::new(guard))
    }
}

pub struct DynLockWriteGuard<'a, T: ?Sized>(Box<dyn LockWriteGuardSurrogate<'a, T> + 'a>);

impl<T: ?Sized> Deref for DynLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T: ?Sized> DerefMut for DynLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.0.as_mut()
    }
}

impl<'a, T: ?Sized> LockWriteGuardlike<'a, T> for DynLockWriteGuard<'a, T> {
    fn downgrade(self) -> DynLockReadGuard<'a, T> {
        self.0.downgrade_box()
    }
}

impl<'a, T: ?Sized, M: Moderator> From<LockWriteGuard<'a, T, M>> for DynLockWriteGuard<'a, T> {
    fn from(guard: LockWriteGuard<'a, T, M>) -> Self {
        DynLockWriteGuard(Box::new(guard))
    }
}

#[derive(Debug)]
pub enum ModeratorKind {
    ReadBiased,
    WriteBiased,
    ArrivalOrdered,
}

pub const MODERATOR_KINDS: [ModeratorKind; 3] = [
    ModeratorKind::ReadBiased,
    ModeratorKind::WriteBiased,
    ModeratorKind::ArrivalOrdered,
];

impl ModeratorKind {
    pub fn make_lock_for_test<T: Sync + Send + 'static>(&self, t: T) -> LockBoxSized<T> {
        println!("test running with moderator {:?}", self);
        match self {
            ModeratorKind::ReadBiased => Box::new(PolyLock(XLock::<_, ReadBiased>::new(t))),
            ModeratorKind::WriteBiased => Box::new(PolyLock(XLock::<_, WriteBiased>::new(t))),
            ModeratorKind::ArrivalOrdered => Box::new(PolyLock(XLock::<_, ArrivalOrdered>::new(t))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::xlock::locklike::{LockBoxSized, LockReadGuardlike, LockWriteGuardlike, Locklike, MODERATOR_KINDS};
    use crate::xlock::{ReadBiased, XLock};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn conformance() {
        let lock = XLock::<_, ReadBiased>::new(0);
        takes_borrowed(&lock);

        takes_owned(lock);

        takes_owned_alt(XLock::<_, ReadBiased>::new(0));

        for moderator in MODERATOR_KINDS {
            let lock = moderator.make_lock_for_test(0);
            takes_boxed(lock);
        }
    }

    fn takes_boxed(lock: LockBoxSized<u64>) {
        let guard = lock.read();
        assert_eq!(0, *guard);

        let mut guard = guard.try_upgrade(Duration::ZERO).upgraded().unwrap();
        assert_eq!(0, *guard);
        *guard = 42;

        let guard = guard.downgrade();
        assert_eq!(42, *guard);
    }

    fn takes_borrowed<'a, L: Locklike<'a, u64>>(lock: &'a L) {
        let guard = lock.try_read(Duration::ZERO).unwrap();
        assert_eq!(0, *guard);

        let mut guard = guard.try_upgrade(Duration::ZERO).upgraded().unwrap();
        assert_eq!(0, *guard);
        *guard = 42;

        let guard = guard.downgrade();
        assert_eq!(42, *guard);

        drop(guard);

        let mut guard = lock.try_write(Duration::ZERO).unwrap();
        assert_eq!(42, *guard);
        *guard = 69;

        let guard = guard.downgrade();
        assert_eq!(69, *guard);
    }

    fn takes_owned<L>(lock: L)
    where
        for<'a> L: Locklike<'a, u64> + 'static,
    {
        let arc = Arc::new(lock);
        thread::spawn(move || {
            arc.try_read(Duration::ZERO);
        })
        .join()
        .unwrap();
    }

    fn takes_owned_alt<L: for<'a> Locklike<'a, u64> + 'static>(lock: L) {
        let arc = Arc::new(lock);
        thread::spawn(move || {
            arc.try_read(Duration::ZERO);
        })
        .join()
        .unwrap();
    }
}
