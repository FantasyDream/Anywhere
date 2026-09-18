//! The framework concepts the launcher is built from.
//!
//! `context` turns an object into related objects, `action` describes what can be done
//! with them, `search` matches a query against every candidate source, `ranking` orders
//! what matched, and `session` describes read-only environment facts. None of these
//! know about a concrete feature or a user interface. Platform calls happen only through
//! registered action handlers.
pub mod action;
pub mod context;
pub mod ranking;
pub mod search;
pub mod session;
