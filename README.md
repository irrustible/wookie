# Wookie

[![License](https://img.shields.io/crates/l/wookie.svg)](https://github.com/irrustible/wookie/blob/main/LICENSE)
[![Package](https://img.shields.io/crates/v/wookie.svg)](https://crates.io/crates/wookie)
[![Documentation](https://docs.rs/wookie/badge.svg)](https://docs.rs/wookie)

A very small and fast executor for one Future on one
thread. Lightweight enough to have many of them.

## Status: beta

The api might change, but as far as we know it's correct and it hasn't
crashed the test suite we wrote it for yet.

It is designed for use in test suites, it's obviously terrible as
executors go for production so keep it to development?

## Usage

Wookie is a single future executor. It's primarily useful for test
suites where you want to step through execution and make assertions
about state, but also useful for benchmarks or anywhere else where you
don't need more.

The easiest way to use this is with the `woke!()` macro, which
packages up a future together with an executor for convenient polling
and pins it on the stack:

```
use core::task::Poll;
use wookie::woke;
let future = async { true };
woke!(future);
assert_eq!(unsafe { future.as_mut().poll() }, Poll::Ready(true));
// or in one step...
woke!(future: async { true });
assert_eq!(unsafe { future.as_mut().poll() }, Poll::Ready(true));
```

In some circumstances, you might need a `Waker` without actually
having a Future to execute. For this you can use the `wookie!()` macro
which creates just the executor and pins it to the stack:

```
use core::task::Poll;
use futures_micro::pin;
use wookie::wookie;
wookie!(my_executor);
let future = async { true };
pin!(future);
assert_eq!(unsafe { my_executor.as_mut().poll(&mut future) }, Poll::Ready(true));
```

## Copyright and License

Copyright (c) 2021 wookie contributors

[Licensed](LICENSE) under Apache License, Version 2.0 (https://www.apache.org/licenses/LICENSE-2.0),
with LLVM Exceptions (https://spdx.org/licenses/LLVM-exception.html).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
licensed as above, without any additional terms or conditions.
