//! One launcher invocation: its environment, its revision and its committed snapshot.
//!
//! The environment is deliberately *not* part of the context graph. The clipboard, the
//! application that was in front and whether Git exists are facts about the invocation that
//! availability rules read; putting them in the graph would make them look like things the
//! user is pointing at.
pub mod facts;
pub mod manager;
pub mod model;

pub use model::LauncherSession;
