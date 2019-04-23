# Escape From Rust

A library to do character and string escaping according to the Rust language
specification. Hopefully, a part of the compiler soon.

Specifically, it aims to unify these two bits:

https://github.com/rust-lang/rust/blob/c21fbfe7e310b9055ed6b7c46b7d37b831a516e3/src/libsyntax/parse/lexer/mod.rs#L928-L1065

https://github.com/rust-lang/rust/blob/c21fbfe7e310b9055ed6b7c46b7d37b831a516e3/src/libsyntax/parse/mod.rs#L313-L366
