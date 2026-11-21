# rust kernel diagnostics
Sat Nov 21 13:49:11 UTC 2026

## clippy
```
[1m[92m    Checking[0m odderon-kernel v0.1.0 (/home/runner/work/odderon/odderon/kernel)
[1m[92m    Finished[0m `dev` profile [unoptimized + debuginfo] target(s) in 4.36s
```

## cargo test
```
[1m[92m   Compiling[0m odderon-kernel v0.1.0 (/home/runner/work/odderon/odderon/kernel)
[1m[92m    Finished[0m `test` profile [unoptimized + debuginfo] target(s) in 2.35s
[1m[92m     Running[0m unittests src/lib.rs (target/debug/build/odderon-kernel/7b83d1ec3af6b84f/out/odderon_kernel-7b83d1ec3af6b84f)

running 7 tests
test tests::closing_the_curtain_reduces_the_draw ... ok
test tests::exp_and_ln_round_trip ... ok
test tests::exp_is_exact_at_zero ... ok
test tests::the_digest_is_stable ... ok
sin vs libm, worst over |x| < 7 rad: 6.928235762870827e-12
test tests::the_room_cycles_instead_of_saturating ... ok
test tests::sin_is_accurate ... ok
test tests::the_sun_is_where_the_sun_is ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[1m[92m     Running[0m tests/parity.rs (target/debug/build/odderon-kernel/e2586b904a367cf8/out/parity-e2586b904a367cf8)

running 1 test
parity: worst sin 0e0, worst room 0e0 K over 13 samples
test the_js_kernel_and_the_rust_kernel_agree ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```
