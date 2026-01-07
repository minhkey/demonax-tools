//! Core library for Demonax game server metadata management.

pub mod database;
pub mod error;
pub mod file_utils;
pub mod parsers;
pub mod processors;
pub mod models;

pub use error::{Result, DemonaxError};