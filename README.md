# RustBank

Short demo of a Rust bank with that can batch process transactions in CSV format.

Bank module has unit test coverage, manual e2e testing/ type&borrow checker to ensure correctness. Leveraged interior mutability which bypass the borrow checker at runtime with RefCell. This allow us to have a immutable Bank struct with mutable accounts & transactions state.

## Prerequisites

The things you need before installing the software.

* rustc 1.62.1 (e092d0b6b 2022-07-16)`

## Usage

A few examples of useful commands and/or tasks.

```
$ cargo run -- transactions.csv > accounts.csv
```