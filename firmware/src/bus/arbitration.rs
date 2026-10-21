//! Queue bus clients without waking other waiters before the owner releases the bus.

use core::ops::{Deref, DerefMut};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    mutex::{Mutex, MutexGuard},
    semaphore::{FairSemaphore, Semaphore, SemaphoreReleaser},
};

pub struct Arbitrated<M: RawMutex, T, const N: usize> {
    value: Mutex<M, T>,
    permits: FairSemaphore<M, N>,
}

pub struct Guard<'a, M: RawMutex, T, const N: usize> {
    // Release the value before the permit wakes the next client.
    value: MutexGuard<'a, M, T>,
    _permit: SemaphoreReleaser<'a, FairSemaphore<M, N>>,
}

impl<M: RawMutex, T, const N: usize> Arbitrated<M, T, N> {
    pub const fn new(value: T) -> Self {
        Self {
            value: Mutex::new(value),
            permits: FairSemaphore::new(1),
        }
    }

    pub async fn lock(&self) -> Guard<'_, M, T, N> {
        let permit = self.permits.acquire(1).await.expect("bus wait queue full");
        self.guard(permit)
    }

    pub fn try_lock(&self) -> Option<Guard<'_, M, T, N>> {
        self.permits.try_acquire(1).map(|permit| self.guard(permit))
    }

    pub fn lock_blocking(&self, mut retry: impl FnMut() -> bool) -> Option<Guard<'_, M, T, N>> {
        loop {
            if let Some(guard) = self.try_lock() {
                return Some(guard);
            }
            if !retry() {
                return None;
            }
        }
    }

    fn guard<'a>(
        &'a self,
        permit: SemaphoreReleaser<'a, FairSemaphore<M, N>>,
    ) -> Guard<'a, M, T, N> {
        Guard {
            value: self
                .value
                .try_lock()
                .expect("bus permit requires exclusive ownership"),
            _permit: permit,
        }
    }
}

impl<M: RawMutex, T, const N: usize> Deref for Guard<'_, M, T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<M: RawMutex, T, const N: usize> DerefMut for Guard<'_, M, T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
