//! Adapter around the vendored, pinned Samsung `tizen-sdk-skills` CLI scripts (Node.js).
//!
//! * Only a fixed allow-list of scripts can be started ([`commands::Script`]).
//! * Arguments are built from validated, typed values and passed as an argv vector — never
//!   through a shell.
//! * Secrets are never passed on the command line (see iteration 2 for certificates).

pub mod commands;
pub mod envelope;
pub mod results;
pub mod runner;
