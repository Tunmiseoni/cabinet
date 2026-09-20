use std::sync::{Mutex, MutexGuard};

pub trait MutexExt<T> {
    fn lock_or_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_or_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|err| {
            log::warn!("recovering from poisoned mutex: {err}");
            err.into_inner()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn recovers_a_poisoned_lock() {
        let mutex = Arc::new(Mutex::new(0u32));
        let poisoned = Arc::clone(&mutex);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.lock().unwrap();
            panic!("poison it");
        })
        .join();

        *mutex.lock_or_recover() += 1;
        assert_eq!(*mutex.lock_or_recover(), 1);
    }
}
