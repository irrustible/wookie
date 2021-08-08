use core::future::Future;
use core::mem::ManuallyDrop;
use core::pin::Pin;
use core::task::{Context, Poll};
use dummy_waker::dummy_waker;

/// A single-stepping executor whose waker does absolutely nothing,
/// but quite quickly.
///
/// ## Example
///
/// ```
/// use core::task::Poll;
/// use wookie::dummy;
/// dummy!(future: async { true });
/// assert_eq!(future.poll(), Poll::Ready(true));
/// ```
#[repr(transparent)]
pub struct Dummy<F>(F);

impl<F: Future> Dummy<F> {
    #[doc(hidden)]
    #[inline(always)]
    pub fn new(future: F) -> Self{ Dummy(future) }

    /// Polls the contained future once.
    ///
    /// ## Example
    ///
    /// ```
    /// use core::task::Poll;
    /// use wookie::dummy;
    /// dummy!(future: async { true });
    /// assert_eq!(future.poll(), Poll::Ready(true)); 
    /// ```
    #[inline(always)]
    pub fn poll(
        self: &mut Pin<&mut Self>
    ) -> Poll<<F as Future>::Output> {
        let this = self.as_mut().project();
        let waker = ManuallyDrop::new(dummy_waker());
        let future = unsafe { Pin::new_unchecked(&mut this.0) };
        let mut ctx = Context::from_waker(&waker);
        Future::poll(future, &mut ctx)
    }

    #[inline(always)]
    fn project(self: Pin<&mut Self>) -> &mut Self {
        unsafe { Pin::into_inner_unchecked(self) }
    }
}

/// Wraps a future in a single-stepping executor whose waker does
/// nothing and pins it to the stack.
///
/// ## Example
///
/// ```
/// use core::task::Poll;
/// use wookie::dummy;
/// dummy!(future: async { true });
/// assert_eq!(future.poll(), Poll::Ready(true));
/// ```
#[cfg(feature="alloc")]
#[macro_export]
macro_rules! dummy {
    ($name:ident) => {
        let mut $name = unsafe { $crate::Dummy::new($name) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    };
    ($name:ident : $future:expr) => {
        let mut $name = unsafe { $crate::Dummy::new($future) };
        #[allow(unused_mut)]
        let mut $name = unsafe { core::pin::Pin::new_unchecked(&mut $name) };
    }
}
