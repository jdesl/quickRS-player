//! The unit-test binary's allocator: it counts the allocator calls made on a thread while that thread is watched,
//! so a test can hold audio-thread code to no allocation and no freeing (the pedalboard, the jam).

use std::cell::Cell;

struct Counting;

thread_local! {
    static WATCHED: Cell<bool> = const { Cell::new(false) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    if WATCHED.with(|w| w.get()) {
        CALLS.with(|c| c.set(c.get() + 1));
    }
}

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        count();
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        count();
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        count();
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Start counting this thread's allocator calls (from zero).
pub fn watch() {
    CALLS.with(|c| c.set(0));
    WATCHED.with(|w| w.set(true));
}

/// Stop counting; the calls made since `watch`.
pub fn calls() -> usize {
    WATCHED.with(|w| w.set(false));
    CALLS.with(|c| c.get())
}
