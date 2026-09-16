//! The framework concepts the launcher is built from.
//!
//! `context` turns an object into related objects, `action` describes what can be done
//! with them, `search` matches a query against every candidate source, `ranking` orders
//! what matched, and `session` owns the state of one launcher invocation. None of these
//! know about a concrete feature, a user interface or a platform API.
pub mod action;
pub mod context;
pub mod ranking;
pub mod search;
pub mod session;
