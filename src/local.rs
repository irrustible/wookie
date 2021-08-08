use crate::*;
use core::cell::Cell;
use core::future::Future;
#[cfg(feature="alloc")]
use core::mem::ManuallyDrop;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// Wraps a future in a single-stepping executor that tracks wakers
/// and pins it on the stack.
///
/// Unlike [`wookie!`], does not require a global allocator at
/// the cost of unsafe polling.
///
/// ## Examples
///
/// ```
/// use core::task::Poll;
/// use wookie::local;
/// local!(future: async { true });
/// assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
///
/// // you can also just give a variable name if you have one:
/// let future = async { true };
/// local!(future);
/// assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
///
/// // we can find out about the state of wakers any time:
/// assert_eq!(future.cloned(), 0);
/// assert_eq!(future.dropped(), 0);
/// assert_eq!(future.woken(), 0);
/// // or equivalently...
/// future.stats().assert(0, 0, 0);
/// ```
#[macro_export]
macro_rules! local {
    ($name:ident) => {
        let mut $name = unsafe { $crate::Local::new($name) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    };
    ($name:ident : $future:expr) => {
        let mut $name = unsafe { $crate::Local::new($future) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    }
}

/// An allocator-less single-future stepping executor for test suites
/// that tracks wakers.
///
/// Unlike [`Wookie`], does not require a global allocator at the cost
/// of unsafe polling.
///
/// ## Examples
///
/// ```
/// use core::task::Poll;
/// use wookie::local;
/// local!(future: async { true });
/// assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
///
/// // you can also just give a variable name if you have one:
/// let future = async { true };
/// local!(future);
/// assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
///
/// // we can find out about the state of wakers any time:
/// assert_eq!(future.cloned(), 0);
/// assert_eq!(future.dropped(), 0);
/// assert_eq!(future.woken(), 0);
/// // or equivalently...
/// future.stats().assert(0, 0, 0);
/// ```
pub struct Local<F> {
    wakey: Wakey,
    future: F,
}

impl<F: Future> Local<F> {
    /// Creates a new [`Local`] without pinning it to the stack. You
    /// probably want the [`local!`] macro.
    #[inline(always)]
    pub fn new(future: F) -> Local<F> {
        let wakey = Wakey::default();
        Local { wakey, future }
    }

    /// Returns how many times the waker has been woken. This count is
    /// cumulative, it is never reset and is allowed to overflow.
    #[inline(always)]
    pub fn woken(self: &mut Pin<&mut Self>) -> u16 {
        self.as_mut().project().wakey.woken.get()
    }

    /// Returns how many times the waker has been cloned. This count is
    /// cumulative, it is never reset and is allowed to overflow.
    #[inline(always)]
    pub fn cloned(self: &mut Pin<&mut Self>) -> u16 {
        self.as_mut().project().wakey.cloned.get()
    }

    /// Returns how many times a clone of the waker has been
    /// dropped. This count is cumulative, it is never reset and is
    /// allowed to overflow.
    #[inline(always)]
    pub fn dropped(self: &mut Pin<&mut Self>) -> u16 {
        self.as_mut().project().wakey.dropped.get()
    }

    /// Returns statistics about use of our wakers.
    #[inline(always)]
    pub fn stats(self: &mut Pin<&mut Self>) -> Stats {
        let wakey = &self.as_mut().project().wakey;
        Stats {
            cloned:  wakey.cloned.get(),
            dropped: wakey.dropped.get(),
            woken:   wakey.woken.get(),
        }
    }
    /// Returns how many times a clone of the waker has been
    /// dropped. This count is cumulative, it is never reset and is
    /// allowed to overflow.
    #[inline(always)]
    pub fn live(self: &mut Pin<&mut Self>) -> u16 {
        let wakey = &self.as_mut().project().wakey;
        wakey.cloned.get() - wakey.dropped.get()
    }

    /// Polls the contained future once.
    ///
    /// ## Example
    ///
    /// ```
    /// use core::task::Poll;
    /// use wookie::local;
    /// local!(future: async { true });
    /// assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
    /// ```
    ///
    /// ## Safety
    ///
    /// You must not allow the Waker the future is polled with to
    /// exist longer than `self`.
    #[inline(always)]
    pub unsafe fn poll(
        self: &mut Pin<&mut Self>
    ) -> Poll<<F as Future>::Output> {
        let this = self.as_mut().project();
        let waker = ManuallyDrop::new(this.waker());
        let future = Pin::new_unchecked(&mut this.future);
        let mut ctx = Context::from_waker(&waker);
        Future::poll(future, &mut ctx)
    }

    /// Polls the contained future to completion, so long as the
    /// previous poll caused one or more wakes.
    ///
    /// ## Example
    ///
    /// ```
    /// use core::task::Poll;
    /// use wookie::local;
    /// local!(future: async { true });
    /// assert_eq!(unsafe { future.poll_while_woken() }, Poll::Ready(true));
    /// ```
    ///
    /// ## Safety
    ///
    /// You must not allow the Waker the future is polled with to
    /// exist longer than `self`.
    #[inline(always)]
    pub unsafe fn poll_while_woken(
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
        let raw = raw_waker(&self.wakey as *const Wakey);
        unsafe { Waker::from_raw(raw) }
    }

    #[inline(always)]
    fn project(self: Pin<&mut Self>) -> &mut Self {
        unsafe { Pin::into_inner_unchecked(self) }
    }

}

#[derive(Default)]
struct Wakey {
    cloned:  Cell<u16>,
    dropped: Cell<u16>,
    woken:   Cell<u16>,
}

impl Wakey {
    fn bump_cloned(&self)  { self.cloned.set(self.cloned.get() + 1) }
    fn bump_woken(&self)   { self.woken.set(self.woken.get() + 1) }
    fn bump_dropped(&self) { self.dropped.set(self.dropped.get() + 1) }
}

fn raw_waker(wakey: *const Wakey) -> RawWaker {
    fn do_clone(data: *const ()) -> RawWaker {
        unsafe { &*data.cast::<Wakey>() }.bump_cloned();
        raw_waker(data.cast())
    }

    fn do_wake(data: *const ()) {
        do_wake_by_ref(data);
        do_drop(data);
    }

    fn do_wake_by_ref(data: *const ()) {
        unsafe { &*data.cast::<Wakey>() }.bump_woken()
    }

    fn do_drop(data: *const ()) {
        unsafe { &*data.cast::<Wakey>() }.bump_dropped()
    }

    RawWaker::new(
        wakey.cast(),
        &RawWakerVTable::new(do_clone, do_wake, do_wake_by_ref, do_drop)
    )
}
