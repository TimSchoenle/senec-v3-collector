//! The SENEC v3 wire protocol: asking a device for keys, and decoding what comes back.
//!
//! A query to `/lala.cgi` is a JSON object of objects in which every wanted key is mapped to an
//! empty string. The device answers with the same shape, each key now holding either a hex-typed
//! number, a list of them, or one of four refusal words.
//!
//! Finding a device on the network belongs to `senec-discovery`. Publishing what it said belongs
//! to `senec-export`.

pub mod client;
pub mod decode;
pub mod model;
pub mod profile;
