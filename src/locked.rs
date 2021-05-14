/// A wrapper asround [spin::Mutex] to circumvent impl restrictions of Rust.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    /// Creates a new [Locked] wrapping the supplied data.
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    /// Locks the [Locked] and returns a guard that permits mutable access to the inner data.
    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
