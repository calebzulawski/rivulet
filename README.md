<img src="icon.png" alt="icon" height="25" />Rivulet
=======
[![Build Status](https://github.com/calebzulawski/rivulet/workflows/Build/badge.svg?branch=master)](https://github.com/calebzulawski/rivulet/actions)
![Rustc Version 1.56+](https://img.shields.io/badge/rustc-1.56+-lightgray.svg)
[![License](https://img.shields.io/crates/l/rivulet)](https://crates.io/crates/rivulet)
[![Crates.io](https://img.shields.io/crates/v/rivulet)](https://crates.io/crates/rivulet)
[![Rust Documentation](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/rivulet)

Rivulet is a library for creating asynchronous pipelines of contiguous data.

Main features, at a glance:

* **Asynchronous**: Pipeline components are `async`-aware, allowing more control over task priority and data backpressure.
* **Contiguous views**: Data is always contiguous, accessible by a single slice.
* **Modular**: Pipelines can be combined and reused using common interfaces.

## License
Multiversion is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
