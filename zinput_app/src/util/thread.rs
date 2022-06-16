use std::{
    any::Any,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

pub struct ThreadHandle<T> {
    handle: JoinHandle<T>,
    stop: Arc<AtomicBool>,
}

impl<T> ThreadHandle<T>
where
    T: Send + 'static,
{
    pub fn spawn<F>(stop: Arc<AtomicBool>, thread: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let handle = std::thread::spawn(thread);

        ThreadHandle { handle, stop }
    }
    
    pub fn stop(self) -> Result<T, Box<dyn Any + Send>> {
        self.stop.store(true, Ordering::Release);

        self.handle.join()
    }
}
