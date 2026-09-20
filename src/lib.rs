//! Omalogbook: the ship's log for Omahoy.
//!
//! The log is a folder of markdown, one note per day, with GPX tracks beside
//! it. Omalogbook writes its own block in each note and the day's numbers in
//! the front matter, and never touches a word the crew wrote. Nothing here
//! needs a database, a server, or this program to read it again.

pub mod config;
pub mod day;
pub mod entry;
pub mod geo;
pub mod git;
pub mod keel;
pub mod lock;
pub mod preset;
pub mod time;
pub mod track;
pub mod watch;
pub mod wind;
