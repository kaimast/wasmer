// This file contains code from external sources.
// Attributions: https://github.com/wasmerio/wasmer/blob/master/ATTRIBUTIONS.md

//! This is the module that facilitates the usage of Traps
//! in Wasmer Runtime

#[allow(clippy::module_inception)]
mod trap;
mod traphandlers;

pub use trap::Trap;
pub use traphandlers::{
    catch_traps, on_host_stack, raise_lib_trap, raise_user_trap, wasmer_call_trampoline,
    TlsRestore, TrapHandler, TrapHandlerFn,
};
pub use traphandlers::{init_traps, resume_panic};
pub use wasmer_types::TrapCode;
