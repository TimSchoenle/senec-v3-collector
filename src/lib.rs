//! No API. The workspace root is a package only so `release-please` can bump its version.
//!
//! It is configured with `release-type: rust` against `.`, and a `[package]` with no target does
//! not build, so this file is the smallest target that satisfies it. Nothing depends on it.
