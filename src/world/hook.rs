mod interface;

pub use interface::*;
use std::sync::{Arc, LockResult, Mutex, MutexGuard};

pub struct SharedHook<H> {
    inner: Arc<Mutex<H>>,
}

impl<H> Clone for SharedHook<H> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E, H> Hook<E> for SharedHook<H>
where
    H: Hook<E>,
{
    // @todo
}

impl<H> SharedHook<H> {
    pub fn new(hook: H) -> Self {
        Self {
            inner: Arc::new(Mutex::new(hook)),
        }
    }

    pub fn lock(&self) -> LockResult<MutexGuard<'_, H>> {
        self.inner.lock()
    }
}

pub struct HookDelegate<E> {
    hooks: Vec<Box<dyn Hook<E>>>,
}

impl<E> Default for HookDelegate<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Hook<E> for HookDelegate<E> {
    // @todo
}

impl<E> HookDelegate<E> {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn add_hook<H>(&mut self, hook: H)
    where
        H: Hook<E> + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    pub fn add_shared_hook<H>(&mut self, hook: H) -> SharedHook<H>
    where
        H: Hook<E> + 'static,
    {
        let shared = SharedHook::new(hook);

        self.hooks.push(Box::new(shared.clone()));

        shared
    }
}
