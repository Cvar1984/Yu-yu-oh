//! The WebAssembly exports: memory for request text, one call that runs a
//! command, and the last panic message. Built only for `wasm32`.

use std::cell::RefCell;

use crate::Host;

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
    static RESPONSE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static PANIC: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Reserves `len` bytes for the page to write a request into.
#[unsafe(no_mangle)]
pub extern "C" fn ygo_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Runs the UTF-8 command at `ptr..ptr + len` (and frees it); returns the
/// length of the answer, read through `ygo_response_ptr`.
///
/// # Safety
/// `ptr` and `len` must come from one `ygo_alloc(len)` call, with the
/// request written into it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ygo_handle(ptr: *mut u8, len: usize) -> usize {
    std::panic::set_hook(Box::new(|info| {
        PANIC.with(|p| *p.borrow_mut() = info.to_string());
    }));
    // SAFETY: the page wrote `len` bytes into the buffer `ygo_alloc(len)`
    // returned; taking it back as a Vec frees it when this call ends.
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    let request = String::from_utf8_lossy(&bytes);
    let answer = HOST.with(|h| h.borrow_mut().handle(&request));
    RESPONSE.with(|r| {
        let mut r = r.borrow_mut();
        *r = answer.into_bytes();
        r.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ygo_response_ptr() -> *const u8 {
    RESPONSE.with(|r| r.borrow().as_ptr())
}

/// The message of the last panic, written into the response buffer;
/// returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn ygo_panic_message() -> usize {
    let message = PANIC.with(|p| p.borrow().clone());
    RESPONSE.with(|r| {
        let mut r = r.borrow_mut();
        *r = message.into_bytes();
        r.len()
    })
}
