//! A very small and fast executor for one Future on one
//! thread. Lightweight enough to have many of them.
//!
//! Primarily useful for test suites where you want to step through
//! execution and make assertions about state, but also useful for
//! benchmarks or anywhere else where you don't need more.
//!
//! The easiest way to use this is with the [`woke!()`] macro, which
//! packages up a future together with an executor for convenient
//! polling and pins it on the stack:
//!
//! ```
//! use core::task::Poll;
//! use wookie::woke;
//! let future = async { true };
//! woke!(future);
//! assert_eq!(unsafe { future.as_mut().poll() }, Poll::Ready(true));
//! // or in one step...
//! woke!(future: async { true });
//! assert_eq!(unsafe { future.as_mut().poll() }, Poll::Ready(true));
//! ```
//!
//! In some circumstances, you might need a [`Waker`] without actually
//! having a Future to execute. For this you can use the [`wookie!()`]
//! macro which creates just the executor and pins it to the stack:
//!
//! ```
//! use core::task::Poll;
//! use futures_micro::pin;
//! use wookie::wookie;
//! wookie!(my_executor);
//! let future = async { true };
//! pin!(future);
//! assert_eq!(unsafe { my_executor.as_mut().poll(&mut future) }, Poll::Ready(true));
//! ```
//!
//! Warning: I mean it when I say this is for one thread. I can't tell
//! you exactly what will happen if you wake it on another thread, but
//! it probably won't be good!
use core::future::Future;
use core::mem::transmute;
use core::pin::Pin;
use core::ptr::{null, null_mut};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use pin_project_lite::pin_project;

const VTABLE: RawWakerVTable =
    RawWakerVTable::new(
        |data: *const ()| RawWaker::new(data, &VTABLE), // clone
        |data: *const ()| unsafe { // wake
            let d: *const *mut u32 = transmute(data);
            **d += 1;
        },
        |data: *const ()| unsafe { // wake by ref
            let d: *const *mut u32 = transmute(data);
            **d += 1;
        },
        |_data: *const ()| (), //drop
    );

pin_project! {
    /// A very small and fast executor for one Future on one thread.
    ///
    /// Primarily designed for test suites where you want to step
    /// through execution and make assertions about state.
    pub struct Wookie {
        counter: u32,
        counter_ref: *mut u32,
        counter_ref_ref: *const *mut u32,
    }
}
    
impl Wookie {
    #[doc(hidden)]
    #[inline(always)]
    pub fn new() -> Wookie {
        Wookie {
            counter: 0,
            counter_ref: null_mut(),
            counter_ref_ref: null(),
        }
    }
    #[doc(hidden)]
    #[inline(always)]
    // Makes our internal pointers actually point. Called by
    // `wookie!()` and `woke!()` after pinning.
    pub fn repoint(self: Pin<&mut Self>) {
        let this = self.project();
        *this.counter_ref = this.counter as *mut u32;
        *this.counter_ref_ref = this.counter_ref as *const *mut u32;
    }
}

impl Wookie {

    /// Returns how many times the waker has been woken. This count is
    /// cumulative, it is never reset.
    ///
    /// Safe if:
    ///   * We have repointed (i.e. you constructed us with [`wookie!()`].
    ///   * A Waker we created won't be woken on another thread.
    ///   * You do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn count(self: Pin<&mut Self>) -> u32 {
        *self.project().counter
    }

    /// Creates a new Waker that will mark our future as to be woken
    /// next poll.
    ///
    /// Safe if:
    ///   * We have repointed (i.e. you constructed us with [`wookie!()`].
    ///   * A Waker we created won't be woken on another thread.
    ///   * you do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn waker(self: Pin<&mut Self>) -> Waker {
        Waker::from_raw(RawWaker::new(transmute(*self.project().counter_ref_ref), &VTABLE))
    }

    /// Polls the provided pinned future once.
    /// 
    /// Safe if you do not send the Waker to another thread.
    /// (Waker is Send and we are not so it's an lies!)
    #[inline(always)]
    pub unsafe fn poll<T>(
        self: Pin<&mut Self>,
        future: &mut Pin<&mut impl Future<Output=T>>
    ) -> Poll<T>
    where {
        let waker = self.waker();
        let mut context = Context::from_waker(&waker);
        future.as_mut().poll(&mut context)
    }

    /// Polls the provided pinned future so long as the previous
    /// poll caused one or more wakes.
    /// 
    /// Safe if:
    ///   * you do not send the Waker to another thread (Waker is Send
    ///     and we are not so it's an lies!)
    ///   * you do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn poll_while_woken<T>(
        self: Pin<&mut Self>,
        future: &mut Pin<&mut impl Future<Output=T>>
    ) -> Poll<T> {
        let this = self.project();
        let waker = Waker::from_raw(RawWaker::new(transmute(*this.counter_ref_ref), &VTABLE));
        let mut context = Context::from_waker(&waker);
        loop {
            let count = *this.counter;
            if let Poll::Ready(r) = future.as_mut().poll(&mut context) { return Poll::Ready(r); }
            if *this.counter == count { return Poll::Pending; }
        }
    }
}

/// Creates a new [`Wookie`], pinned on the stack under the given name.
///
/// # Examples
///
/// ```
/// use core::task::Poll;
/// use futures_micro::pin;
/// use wookie::wookie;
/// wookie!(my_executor);
/// let future = async { true };
/// pin!(future);
/// assert_eq!(unsafe { my_executor.as_mut().poll(&mut future) }, Poll::Ready(true));
/// ```
#[macro_export]
macro_rules! wookie {
    ($name:ident) => {
        let mut $name = unsafe { $crate::Wookie::new() };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
        $name.as_mut().repoint();
    }
}

pin_project! {
    /// A single future and its lightweight executor
    pub struct Woke<F> {
        #[pin]
        wookie: Wookie,
        #[pin]
        future: F,
    }
}

impl<F: Future> Woke<F> {

    #[doc(hidden)]
    #[inline(always)]
    pub fn repoint(self: Pin<&mut Self>) {
        self.project().wookie.repoint();
    }

    /// Returns how many times the waker has been woken. This count is
    /// cumulative, it is never reset.
    ///
    /// Safe provided a Waker we created won't be woken on another thread.
    #[inline(always)]
    pub unsafe fn count(self: Pin<&mut Self>) -> u32 {
        self.project().wookie.count()
    }

    /// Creates a new Waker that will mark our future as to be woken
    /// next poll.
    ///
    /// Safe if:
    ///   * you do not send the Waker to another thread (Waker is Send
    ///     and we are not so it's an lies!)
    ///   * you do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn waker(self: Pin<&mut Self>) -> Waker {
        self.project().wookie.waker()
    }

    /// Polls the contained future once.
    /// 
    /// Safe if you do not send the Waker to another thread.
    /// (Waker is Send and we are not so it's an lies!)
    #[inline(always)]
    pub unsafe fn poll(
        self: Pin<&mut Self>
    ) -> Poll<<F as Future>::Output> {
        let mut this = self.project();
        this.wookie.as_mut().poll(&mut this.future)
    }

    /// Polls the contained future so long as the previous poll caused
    /// one or more wakes.
    /// 
    /// Safe if:
    ///   * you do not send the Waker to another thread (Waker is Send
    ///     and we are not so it's an lies!)
    ///   * you do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn poll_while_woken(
        self: Pin<&mut Self>
    ) -> Poll<<F as Future>::Output> {
        let mut this = self.project();
        this.wookie.as_mut().poll_while_woken(&mut this.future)
    }

    #[doc(hidden)]
    #[inline(always)]
    pub unsafe fn new(future: F) -> Woke<F> {
        Woke { future, wookie: Wookie::new() }
    }
}

/// Parcel a [`Future`] with a [`Wookie`] to execute it and pin it to the
/// stack.
///
/// # Examples
///
/// ```
/// use core::task::Poll;
/// use wookie::woke;
/// let future = async { true };
/// woke!(future);
/// assert_eq!(unsafe { future.as_mut().poll() }, Poll::Ready(true));
/// // or in one step...
/// woke!(future: async { true });
/// assert_eq!(unsafe { future.as_mut().poll() }, Poll::Ready(true));
/// ```
#[macro_export]
macro_rules! woke {
    ($name:ident) => {
        let mut $name = unsafe { $crate::Woke::new($name) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
        $name.as_mut().repoint();
    };
    ($name:ident : $future:expr) => {
        let mut $name = unsafe { $crate::Woke::new($future) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
        $name.as_mut().repoint();
    }
}
