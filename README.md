# Overview

`ba2` is a DOM-based reader/writer for archives made for the Creation Engine games. It includes near-complete support for all archive variants from Morrowind up to Starfield. `ba2` leverages mem-mapped i/o to cut down on the memory bloat typically associated with DOM-based approaches. It is Rust a port of the equivalent [C++ library](https://github.com/Ryan-rsm-McKenzie/bsa).

The latest development docs are available at: https://ryan-rsm-mckenzie.github.io/bsa-rs/ba2/index.html

The stable release docs are available at: https://docs.rs/ba2/latest/ba2/

Changelogs are available on the Github releases page: https://github.com/Ryan-rsm-McKenzie/bsa-rs/releases

# Maturity

`ba2` is not nearly as mature as its C++ cousin, however it does leverage the C++ test suite, and as such it manages to stand head and shoulders above existing solutions in terms of correctness of implementation. Tests are written directly in the source code, instead of being kept separately. See [here](https://github.com/Ryan-rsm-McKenzie/bsa-rs/blob/51521859898fc67e24c7783a31c35ce66d5b9559/src/tes3/archive.rs#L244), [here](https://github.com/Ryan-rsm-McKenzie/bsa-rs/blob/51521859898fc67e24c7783a31c35ce66d5b9559/src/tes4/archive.rs#L906), and [here](https://github.com/Ryan-rsm-McKenzie/bsa-rs/blob/51521859898fc67e24c7783a31c35ce66d5b9559/src/fo4/archive.rs#L574) for the majority of the written tests.

# Release Schedule

Being an implementation of a relatively static archive file format, `ba2` has no set release schedule. However updates will be published sporadically as bug fixes are made, new features are added, and especially when new game releases necessitate support.

# Contributing

Contributions are generally welcome, however I would appreciate if contributions were discussed beforehand using Github issues, before any code is written.
