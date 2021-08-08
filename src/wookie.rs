use super::Stats;
use alloc::sync::Arc;
use core::future::Future;
use core::mem::ManuallyDrop;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use core::sync::atomic::{AtomicU16, Ordering::Relaxed};

/// A single-future stepping executor for test suites that tracks wakers.
///
/// ## Examples
///
/// ```
/// use core::task::Poll;
/// use wookie::wookie;
/// wookie!(future: async { true });
/// assert_eq!(future.poll(), Poll::Ready(true));
///
/// // you can also just give a variable name if you have one:
/// let future = async { true };
/// wookie!(future);
/// assert_eq!(future.poll(), Poll::Ready(true));
///
/// // we can find out about the state of wakers any time:
/// assert_eq!(future.cloned(), 0);
/// assert_eq!(future.dropped(), 0);
/// assert_eq!(future.woken(), 0);
/// // or equivalently...
/// future.stats().assert(0, 0, 0);
/// ```
pub struct Wookie<F> {
    wakey: Arc<Wakey>,
    ptr: *const Wakey,
    future: F,
}


impl<F: Future> Wookie<F> {

    /// Creates a new [`Wookie`] without pinning it to the stack. You
    /// probably want the [`crate::wookie!`] macro.
    #[inline(always)]
    pub fn new(future: F) -> Wookie<F> {
        let ptr = Arc::into_raw(Arc::new(Wakey::default()));
        let wakey = unsafe { Arc::from_raw(ptr) };
        Wookie { wakey, ptr, future }
    }

    /// Returns how many times the waker has been woken. This count is
    /// cumulative, it is never reset and is allowed to overflow.
    #[inline(always)]
    pub fn woken(self: &mut Pin<&mut Self>) -> u16 {
        self.as_mut().project().wakey.woken.load(Relaxed)
    }

    /// Returns how many times the waker has been cloned. This count is
    /// cumulative, it is never reset and is allowed to overflow.
    #[inline(always)]
    pub fn cloned(self: &mut Pin<&mut Self>) -> u16 {
        self.as_mut().project().wakey.cloned.load(Relaxed)
    }

    /// Returns how many times a clone of the waker has been
    /// dropped. This count is cumulative, it is never reset and is
    /// allowed to overflow.
    #[inline(always)]
    pub fn dropped(self: &mut Pin<&mut Self>) -> u16 {
        self.as_mut().project().wakey.dropped.load(Relaxed)
    }

    /// Returns statistics about use of our wakers.
    #[inline(always)]
    pub fn stats(self: &mut Pin<&mut Self>) -> Stats {
        let wakey = self.as_mut().project().wakey.as_ref();
        Stats {
            cloned:  wakey.cloned.load(Relaxed),
            dropped: wakey.dropped.load(Relaxed),
            woken:   wakey.woken.load(Relaxed),
        }
    }
    /// Returns how many times a clone of the waker has been
    /// dropped. This count is cumulative, it is never reset and is
    /// allowed to overflow.
    #[inline(always)]
    pub fn live(self: &mut Pin<&mut Self>) -> u16 {
        let wakey = self.as_mut().project().wakey.as_ref();
        wakey.cloned.load(Relaxed) - wakey.dropped.load(Relaxed)
    }

    /// Polls the contained future once.
    ///
    /// ## Example
    ///
    /// ```
    /// use core::task::Poll;
    /// use wookie::wookie;
    /// wookie!(future: async { true });
    /// assert_eq!(future.poll(), Poll::Ready(true));
    /// ```
    #[inline(always)]
    pub fn poll(
        self: &mut Pin<&mut Self>
    ) -> Poll<<F as Future>::Output> {
        let this = self.as_mut().project();
        let waker = ManuallyDrop::new(this.waker());
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        let mut ctx = Context::from_waker(&waker);
        Future::poll(future, &mut ctx)
    }

    /// Polls the contained future until completion, so long as the
    /// previous poll caused one or more wakes.
    ///
    /// ## Example
    ///
    /// ```
    /// use core::task::Poll;
    /// use wookie::wookie;
    /// wookie!(future: async { true });
    /// assert_eq!(future.poll_while_woken(), Poll::Ready(true));
    /// ```
    #[inline(always)]
    pub fn poll_while_woken(
        self: &mut Pin<&mut Self>
    ) -> Poll<<F as Future>::Output> {
        let mut woken = self.woken();
        loop {
            if let Poll::Ready(r) = self.poll() { return Poll::Ready(r); }
            let w = self.woken();
            if w == woken { return Poll::Pending; }
            woken = w;
        }
    }

    #[inline(always)]
    fn waker(&self) -> Waker {
        // Safety: the returned waker is valid as long as self is
        // valid. But in order to do anything mutable with the Waker,
        // they would have to have cloned it first.
        let raw = wookie_rawwaker(self.ptr);
        unsafe { Waker::from_raw(raw) }
    }

    #[inline(always)]
    fn project(self: Pin<&mut Self>) -> &mut Self {
        unsafe { Pin::into_inner_unchecked(self) }
    }

}


/// Wraps a future in a single-future stepping executor for test
/// suites that tracks wakers and pins it on the stack.
///
/// ## Examples
///
/// ```
/// use core::task::Poll;
/// use wookie::wookie;
/// wookie!(future: async { true });
/// assert_eq!(future.poll(), Poll::Ready(true));
///
/// // you can also just give a variable name if you have one:
/// let future = async { true };
/// wookie!(future);
/// assert_eq!(future.poll(), Poll::Ready(true));
///
/// // we can find out about the state of wakers any time:
/// assert_eq!(future.cloned(), 0);
/// assert_eq!(future.dropped(), 0);
/// assert_eq!(future.woken(), 0);
/// // or equivalently...
/// future.stats().assert(0, 0, 0);
/// ```
#[macro_export]
macro_rules! wookie {
    ($name:ident) => {
        let mut $name = unsafe { $crate::Wookie::new($name) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    };
    ($name:ident : $future:expr) => {
        let mut $name = unsafe { $crate::Wookie::new($future) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    }
}

#[derive(Default)]
struct Wakey {
    cloned:  AtomicU16,
    dropped: AtomicU16,
    woken:   AtomicU16,
}

impl Wakey {
    fn bump_cloned(&self)  -> u16 { self.cloned.fetch_add(1, Relaxed) }
    fn bump_woken(&self)   -> u16 { self.woken.fetch_add(1, Relaxed) }
    fn bump_dropped(&self) -> u16 { self.dropped.fetch_add(1, Relaxed) }
}

fn wookie_rawwaker(wakey: *const Wakey) -> RawWaker {
    fn do_clone(data: *const ()) -> RawWaker {
        let wakey = data as *const Wakey;
        unsafe { &*wakey }.bump_cloned();
        unsafe { Arc::increment_strong_count(wakey) };
        wookie_rawwaker(wakey)
    }

    fn do_wake(data: *const ()) {
        let wakey: Arc<Wakey> = unsafe { Arc::from_raw(data as *const Wakey) };
        wakey.bump_woken();
        wakey.bump_dropped();
    }

    fn do_wake_by_ref(data: *const ()) {
        let arc = unsafe { Arc::from_raw(data as *const Wakey) };
        let wakey = ManuallyDrop::new(arc);
        wakey.bump_woken();
    }

    fn do_drop(data: *const ()) {
        let wakey: Arc<Wakey> = unsafe { Arc::from_raw(data as *const Wakey) };
        wakey.bump_dropped();
    }

    RawWaker::new(
        wakey as *const (),
        &RawWakerVTable::new(do_clone, do_wake, do_wake_by_ref, do_drop)
    )
}

