use std::sync::OnceLock;
use tokio::runtime::Runtime;

static SHARED_RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Initialize the shared async runtime. Call once at application startup.
/// Panics if called more than once or if Runtime creation fails.
pub fn init() {
    let rt = Runtime::new().expect("Failed to create shared tokio runtime");
    SHARED_RUNTIME
        .set(rt)
        .expect("init() called more than once");
}

/// Get a handle to the shared async runtime.
/// Returns None if init() has not been called yet.
pub fn handle() -> Option<tokio::runtime::Handle> {
    SHARED_RUNTIME.get().map(|rt| rt.handle().clone())
}

/// Run a future on the shared runtime, blocking the current thread.
/// Returns Err with the future if the runtime has not been initialized.
pub fn block_on<F, T>(future: F) -> Result<T, F>
where
    F: std::future::Future<Output = T>,
{
    match SHARED_RUNTIME.get() {
        Some(rt) => Ok(rt.block_on(future)),
        None => Err(future),
    }
}
