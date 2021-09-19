# Wookie

[![License](https://img.shields.io/crates/l/wookie.svg)](https://github.com/irrustible/wookie/blob/main/LICENSE)
[![Package](https://img.shields.io/crates/v/wookie.svg)](https://crates.io/crates/wookie)
[![Documentation](https://docs.rs/wookie/badge.svg)](https://docs.rs/wookie)

Async test/bench toolkit including single stepping executors. No-std compatible.

## Status: beta

We've done a few iterations now and we quite like it how it is now and
believe it to be correct.

## Usage

The primary user interface is the `wookie!` macro, which wraps a
future with an executor and pins it on the stack.:

```rust
use core::task::Poll;
use wookie::wookie;
wookie!(future: async { true });
assert_eq!(future.poll(), Poll::Ready(true));

// you can also just give a variable name if you have one:
let future = async { true };
wookie!(future);
assert_eq!(future.poll(), Poll::Ready(true));

// we can find out about the state of wakers any time:
assert_eq!(future.cloned(), 0);
assert_eq!(future.dropped(), 0);
assert_eq!(future.woken(), 0);
// or equivalently...
future.stats().assert(0, 0, 0);
```

If you do not have access to an allocator, you can use the `local!`
macro instead, however, polling is unsafe and you must be very careful
to maintain the invariants described in the `safety` sections of the
`Local` methods.

```rust
use core::task::Poll;
use wookie::local;
local!(future: async { true });
assert_eq!(unsafe { future.poll() }, Poll::Ready(true));

// you can also just give a variable name if you have one:
let future = async { true };
local!(future);
assert_eq!(unsafe { future.poll() }, Poll::Ready(true));

// we can find out about the state of wakers any time:
assert_eq!(future.cloned(), 0);
assert_eq!(future.dropped(), 0);
assert_eq!(future.woken(), 0);
// or equivalently...
future.stats().assert(0, 0, 0);
```

For benchmarking, we provide the `dummy!` macro, whose waker does
nothing, but quite quickly.

```rust
use core::task::Poll;
use wookie::dummy;
dummy!(future: async { true });
assert_eq!(future.poll(), Poll::Ready(true));
```

We have `assert_pending!` and `assert_ready!` to save some
typing in assertions:

```
use wookie::*;
use core::task::Poll;
assert_pending!(Poll::<i32>::Pending); // pass
// assert_pending!(Poll::Ready(())); // would fail

// With 1 arg, assert_ready will returning the unwrapped value.
assert_eq!(42, assert_ready!(Poll::Ready(42)));
// assert_ready!(Poll::<i32>::Pending); // would fail

// With 2 args, it's like [`assert_eq`] on the unwrapped value.
assert_ready!(42, Poll::Ready(42));
// assert_ready!(Poll::<i32>::Pending); // would fail
// assert_ready!(42, Poll::Ready(420)); // would fail
```

MSRV: 1.51.0

## Features

Default features: `alloc`.

* `alloc` - enables use of an allocator. Required by `Wookie` / `wookie!`.

## Copyright and License

Copyright (c) 2021 James Laver, wookie contributors

[Licensed](LICENSE) under Apache License, Version 2.0 (https://www.apache.org/licenses/LICENSE-2.0),
with LLVM Exceptions (https://spdx.org/licenses/LLVM-exception.html).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
licensed as above, without any additional terms or conditions.
