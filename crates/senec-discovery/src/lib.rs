//! Deriving a device's key list from the web UI it serves.
//!
//! A key the web UI never mentions cannot be found, so what [`discover`] returns is the
//! intersection of what `/js/senec.min.js` and the pages it names know about with what this one
//! device answers for. Two devices on the same firmware can still produce different lists.

mod discovery;

pub use discovery::discover;
