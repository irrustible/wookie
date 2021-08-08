//! Single future stepping executors for test suites and benchmarking.
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
