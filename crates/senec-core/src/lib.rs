//! The SENEC v3 wire protocol: asking a device for keys, and decoding what comes back.
//!
//! A query to `/lala.cgi` is a JSON object of objects in which every wanted key is mapped to an
//! empty string. The device answers with the same shape, each key now holding either a hex-typed
//! number, a list of them, or one of four refusal words. [`client`] performs that exchange,
//! [`decode`] turns the strings into numbers, and [`model`] holds both the key list a caller asks
//! with and the classification of what comes back. [`profile`] reads that list off disk.

pub mod client;
pub mod decode;
pub mod model;
pub mod profile;
