This directory contains the fuzz tests for wasmer. To fuzz, we use the `cargo-fuzz` package.

## Installation

You may need to install the `cargo-fuzz` package to get the `cargo fuzz` subcommand. Use

```sh
$ cargo install cargo-fuzz
```

`cargo-fuzz` is documented in the [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz.html).

## Running a fuzzer (validate, jit_llvm, native_cranelift, ...)

Once `cargo-fuzz` is installed, you can run the `validate` fuzzer with
```sh
cargo fuzz run validate
```
or the `jit_cranelift` fuzzer
```sh
cargo fuzz run jit_cranelift
```
See the [fuzz/fuzz_targets](https://github.com/wasmerio/wasmer/tree/fuzz/fuzz_targets/) directory for the full list of targets.

You should see output that looks something like this:

```
#1408022        NEW    cov: 115073 ft: 503843 corp: 4659/1807Kb lim: 4096 exec/s: 889 rss: 857Mb L: 2588/4096 MS: 1 ChangeASCIIInt-
#1408273        NEW    cov: 115073 ft: 503844 corp: 4660/1808Kb lim: 4096 exec/s: 888 rss: 857Mb L: 1197/4096 MS: 1 ShuffleBytes-
#1408534        NEW    cov: 115073 ft: 503866 corp: 4661/1809Kb lim: 4096 exec/s: 886 rss: 857Mb L: 977/4096 MS: 1 ShuffleBytes-
#1408540        NEW    cov: 115073 ft: 503869 corp: 4662/1811Kb lim: 4096 exec/s: 886 rss: 857Mb L: 2067/4096 MS: 1 ChangeBit-
#1408831        NEW    cov: 115073 ft: 503945 corp: 4663/1811Kb lim: 4096 exec/s: 885 rss: 857Mb L: 460/4096 MS: 1 CMP- DE: "\x16\x00\x00\x00\x00\x00\x00\x00"-
#1408977        NEW    cov: 115073 ft: 503946 corp: 4664/1813Kb lim: 4096 exec/s: 885 rss: 857Mb L: 1972/4096 MS: 1 ShuffleBytes-
#1408999        NEW    cov: 115073 ft: 503949 corp: 4665/1814Kb lim: 4096 exec/s: 884 rss: 857Mb L: 964/4096 MS: 2 ChangeBit-ShuffleBytes-
#1409040        NEW    cov: 115073 ft: 503950 corp: 4666/1814Kb lim: 4096 exec/s: 884 rss: 857Mb L: 90/4096 MS: 1 ChangeBit-
#1409042        NEW    cov: 115073 ft: 503951 corp: 4667/1814Kb lim: 4096 exec/s: 884 rss: 857Mb L: 174/4096 MS: 2 ChangeByte-ChangeASCIIInt-
```

It will continue to generate random inputs forever, until it finds a bug or is terminated. The testcases for bugs it finds go into `fuzz/artifacts/jit_cranelift` and you can rerun the fuzzer on a single input by passing it on the command line `cargo fuzz run jit_cranelift /path/to/testcase`.

## The corpus

Each fuzzer has an individual corpus under fuzz/corpus/test_name, created on first run if not already present. The fuzzers use `wasm-smith` which means that the testcase files are random number seeds input to the wasm generator, not `.wasm` files themselves. In order to debug a testcase, you may find that you need to convert it into a `.wasm` file. Using the standalone `wasm-smith` tool doesn't work for this purpose because we use a custom configuration to our `wasm_smith::Module`. Instead, our fuzzers use an environment variable `DUMP_TESTCASE=path`. For example:

```
DUMP_TESTCASE=/tmp/crash.wasm cargo fuzz run --features=jit,singlepass jit_singlepass fuzz/artifacts/jit_singlepass/crash-0966412eab4f89c52ce5d681807c8030349470f6
```
