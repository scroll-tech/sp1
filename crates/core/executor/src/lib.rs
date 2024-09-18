//! An implementation of an exucutor for the SP1 RISC-V zkVM.

#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::should_panic_without_expect)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::manual_assert)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::explicit_iter_loop)]
#![warn(missing_docs)]

mod context;
mod disassembler;
pub mod events;
mod executor;
mod hook;
mod instruction;
mod io;
mod memory;
mod opcode;
mod program;
#[cfg(any(test, feature = "programs"))]
pub mod programs;
mod record;
mod register;
mod report;
mod state;
pub mod subproof;
pub mod syscalls;
mod utils;

pub use context::*;
pub use executor::*;
pub use hook::*;
pub use instruction::*;
pub use opcode::*;
pub use program::*;
pub use record::*;
pub use register::*;
pub use report::*;
pub use state::*;
