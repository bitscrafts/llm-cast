//! MCP-over-stdio server (spec-03). Part 1 declares only the seams: the
//! process runner, the environment config, the cast port, and the error type.
//! The `McpServer` tool surface lands in part 2.

pub mod cast;
pub mod config;
pub mod errors;
pub mod runner;
