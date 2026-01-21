//! Core library for Demonax game server metadata management.

pub mod database;
pub mod error;
pub mod file_utils;
pub mod harvesting;
pub mod parsers;
pub mod processors;
pub mod models;

pub use error::{Result, DemonaxError};
pub use harvesting::{generate_harvesting_rule, generate_all_harvesting_rules, insert_harvesting_rules};