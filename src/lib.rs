//! Async test/bench toolkit including single stepping executors. No-std compatible.
//!
//! The primary user interface is the [`wookie!`] macro, which wraps a
//! future with an executor and pins it on the stack.:
//!
//! ```
//! use core::task::Poll;
//! use wookie::wookie;
//! wookie!(future: async { true });
//! assert_eq!(future.poll(), Poll::Ready(true));
//!
//! // you can also just give a variable name if you have one:
//! let future = async { true };
//! wookie!(future);
//! assert_eq!(future.poll(), Poll::Ready(true));
//!
//! // we can find out about the state of wakers any time:
//! assert_eq!(future.cloned(), 0);
//! assert_eq!(future.dropped(), 0);
//! assert_eq!(future.woken(), 0);
//! // or equivalently...
//! future.stats().assert(0, 0, 0);
//! ```
//!
//! If you do not have access to an allocator, you can use the [`local!`]
//! macro instead, however, polling is unsafe and you must be very careful to
//! maintain the invariants described in the `safety` sections of the
//! [`Local`] methods.
//!
//! ```
//! use core::task::Poll;
//! use wookie::local;
//! local!(future: async { true });
//! assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
//!
//! // you can also just give a variable name if you have one:
//! let future = async { true };
//! local!(future);
//! assert_eq!(unsafe { future.poll() }, Poll::Ready(true));
//!
//! // we can find out about the state of wakers any time:
//! assert_eq!(future.cloned(), 0);
//! assert_eq!(future.dropped(), 0);
//! assert_eq!(future.woken(), 0);
//! // or equivalently...
//! future.stats().assert(0, 0, 0);
//! ```
//!
//! For benchmarking, we provide the [`dummy!`] macro, whose waker does
//! nothing, but quite quickly.
//!
//! ```
//! use core::task::Poll;
//! use wookie::dummy;
//! dummy!(future: async { true });
//! assert_eq!(future.poll(), Poll::Ready(true));
//! ```
//!
//! We have [`assert_pending!`] and [`assert_ready!`] to save some
//! typing in assertions:
//!
//! ```
//! use wookie::*;
//! use core::task::Poll;
//! assert_pending!(Poll::<i32>::Pending); // pass
//! // assert_pending!(Poll::Ready(())); // would fail
//!
//! // With 1 arg, assert_ready will returning the unwrapped value.
//! assert_eq!(42, assert_ready!(Poll::Ready(42)));
//! // assert_ready!(Poll::<i32>::Pending); // would fail
//!
//! // With 2 args, it's like [`assert_eq`] on the unwrapped value.
//! assert_ready!(42, Poll::Ready(42));
//! // assert_ready!(Poll::<i32>::Pending); // would fail
//! // assert_ready!(42, Poll::Ready(420)); // would fail
//! ```
//!
//! ## Features
//!
//! Default features: `alloc`.
//!
//! * `alloc` - enables use of an allocator. Required by [`Wookie`] / [`wookie!`].
#![no_std]

#[cfg(feature="alloc")]
extern crate alloc;

mod dummy;
#[doc(inline)]
pub use dummy::*;

mod local;
pub use local::*;

#[cfg(feature="alloc")]
mod wookie;
#[cfg(feature="alloc")]
pub use crate::wookie::*;

/// Statistics of waker activity for [`Wookie`] or [`Local`].
pub struct Stats {
    /// The number of times a Waker has been cloned. Usually equivalent to the
    /// number of times a waker has been set.
    pub cloned:  u16,
    /// The number of times a Waker has been dropped. Note that `wake` causes
    /// this count to be incremented as it takes ownership of the Waker.
    pub dropped: u16,
    /// The number of times a Waker has been woken. Includes calls to both
    /// `wake` and `wake_by_ref`.
    pub woken:   u16,
}

impl Stats {
    /// The number of live wakers, i.e. `cloned - dropped`.
    #[inline(always)]
    pub fn live(&self) -> u16 { self.cloned - self.dropped }

    /// Assert that `cloned`, `dropped` and `woken` are the provided values.
    pub fn assert(&self, cloned: u16, dropped: u16, woken: u16) {
        assert_eq!((cloned, dropped, woken), (self.cloned, self.dropped, self.woken));
    }
}

#[macro_export]
/// Asserts that a [`Poll`] is a [`Poll::Pending`]
///
/// ## Examples
///
/// ```
/// use wookie::assert_pending;
/// use core::task::Poll;
/// assert_pending!(Poll::<i32>::Pending); // pass
/// // assert_pending!(Poll::Ready(())); // would fail
/// ```
macro_rules! assert_pending {
    ($expr:expr) => {
        if let Poll::Ready(r) = $expr {
            panic!("Expected Poll::Pending, got Poll::Ready({:?})!", r);
        }
    }
}

#[macro_export]
/// Asserts that a [`Poll`] is a [`Poll::Ready`]
///
/// ## Examples
///
/// ```
/// use wookie::assert_ready;
/// use core::task::Poll;
///
/// // With 1 arg, just checks for ready, returning the unwrapped value.
/// assert_eq!(42, assert_ready!(Poll::Ready(42)));
/// // assert_ready!(Poll::<i32>::Pending); // would fail
///
/// // With 2 args, it's like [`assert_eq`] on the unwrapped value.
/// assert_ready!(42, Poll::Ready(42));
/// // assert_ready!(Poll::<i32>::Pending); // would fail
/// // assert_ready!(42, Poll::Ready(420)); // would fail
/// ```
macro_rules! assert_ready {
    ($expr:expr) => {
        match $expr {
            Poll::Ready(r) => r,
            Poll::Pending => panic!("Expected Poll::Ready, got Poll::Pending!"),
        }
    };
    ($expected:expr, $expr:expr) => {
        match $expr {
            Poll::Ready(r) => assert_eq!($expected, r),
            Poll::Pending => panic!("Expected Poll::Ready, got Poll::Pending!"),
        }
    }
}
