//! A very small and fast executor for one Future. Lightweight enough
//! to have many of them.
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
#![no_std]

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use pin_project_lite::pin_project;

const VTABLE: RawWakerVTable =
    RawWakerVTable::new(
        |data: *const ()| RawWaker::new(data, &VTABLE), // clone
        |data: *const ()| unsafe { // wake
            let d: *const AtomicUsize = data.cast();
            (*d).fetch_add(1, Relaxed);
        },
        |data: *const ()| unsafe { // wake by ref
            let d: *const AtomicUsize = data.cast();
            (*d).fetch_add(1, Relaxed);
        },
        |_data: *const ()| (), //drop
    );

/// A very small and fast executor for one Future on one thread.
///
/// Primarily designed for test suites where you want to step
/// through execution and make assertions about state.
pub struct Wookie {
    counter: AtomicUsize,
}
    
impl Wookie {
    #[doc(hidden)]
    #[inline(always)]
    pub fn new() -> Wookie {
        Wookie { counter: AtomicUsize::new(0) }
    }

    unsafe fn project(self: Pin<&mut Self>) -> &mut Self {
        Pin::get_unchecked_mut(self)
    }
}

impl Wookie {

    /// Returns how many times the waker has been woken. This count is
    /// cumulative, it is never reset.
    ///
    /// # Safety
    ///
    /// Safe if you do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn count(self: Pin<&mut Self>) -> usize {
        self.project().counter.load(Relaxed)
    }

    /// Creates a new Waker that will mark our future as to be woken
    /// next poll.
    ///
    /// # Safety
    ///
    /// Safe if you do not wake after self has dropped
    #[inline(always)]
    pub unsafe fn waker(self: Pin<&mut Self>) -> Waker {
        Waker::from_raw(RawWaker::new((&self.project().counter as *const AtomicUsize).cast(), &VTABLE))
    }

    /// Polls the provided pinned future once.
    /// 
    /// # Safety
    ///
    /// You must not wake after self has dropped.
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
    /// # Safety
    ///
    /// You must not wake after self has dropped.
    #[inline(always)]
    pub unsafe fn poll_while_woken<T>(
        self: Pin<&mut Self>,
        future: &mut Pin<&mut impl Future<Output=T>>
    ) -> Poll<T> {
        let this = self.project();
        let waker = Waker::from_raw(RawWaker::new((&this.counter as *const AtomicUsize).cast(), &VTABLE));
        let mut context = Context::from_waker(&waker);
        loop {
            let count = this.counter.load(Relaxed);
            if let Poll::Ready(r) = future.as_mut().poll(&mut context) { return Poll::Ready(r); }
            if this.counter.load(Relaxed) == count { return Poll::Pending; }
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
#[macro_export ]
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

    /// Returns how many times the waker has been woken. This count is
    /// cumulative, it is never reset.
    ///
    /// # Safety
    ///
    /// You must not wake after self has dropped.
    #[inline(always)]
    pub unsafe fn count(self: Pin<&mut Self>) -> usize {
        self.project().wookie.count()
    }

    /// Creates a new Waker that will mark our future as to be woken
    /// next poll.
    ///
    /// # Safety
    ///
    /// You must not wake after self has dropped.
    #[inline(always)]
    pub unsafe fn waker(self: Pin<&mut Self>) -> Waker {
        self.project().wookie.waker()
    }

    /// Polls the contained future once.
    /// 
    /// # Safety
    ///
    /// You must not wake after self has dropped.
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
    /// # Safety
    ///
    /// You must not wake after self has dropped.
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
    };
    ($name:ident : $future:expr) => {
        let mut $name = unsafe { $crate::Woke::new($future) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    }
}
