//! This are the common types and utility tools for using WebAssembly
//! in a Rust environment.
//!
//! This crate provides common structures such as `Type` or `Value`, type indexes
//! and native function wrappers with `Func`.

#![deny(missing_docs, unused_extern_crates)]
#![warn(unused_import_braces)]
#![cfg_attr(feature = "std", deny(unstable_features))]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
#![cfg_attr(
    feature = "cargo-clippy",
    warn(
        clippy::float_arithmetic,
        clippy::mut_mut,
        clippy::nonminimal_bool,
        clippy::map_unwrap_or,
        clippy::print_stdout,
        clippy::unicode_not_nfc,
        clippy::use_self
    )
)]

#[cfg(all(feature = "std", feature = "core"))]
compile_error!(
    "The `std` and `core` features are both enabled, which is an error. Please enable only once."
);

#[cfg(all(not(feature = "std"), not(feature = "core")))]
compile_error!("Both the `std` and `core` features are disabled. Please enable one of them.");

#[cfg(feature = "core")]
extern crate alloc;

/// The `lib` module defines a `std` module that is identical whether
/// the `core` or the `std` feature is enabled.
pub mod lib {
    /// Custom `std` module.
    #[cfg(feature = "core")]
    pub mod std {
        pub use alloc::{borrow, boxed, format, iter, rc, slice, string, vec};
        pub use core::{any, cell, cmp, convert, fmt, hash, marker, mem, ops, ptr, sync, u32};
    }

    /// Custom `std` module.
    #[cfg(feature = "std")]
    pub mod std {
        pub use std::{
            any, borrow, boxed, cell, cmp, convert, fmt, format, hash, iter, marker, mem, ops, ptr,
            rc, slice, string, sync, u32, vec,
        };
    }
}

pub mod compilation;
pub mod error;
mod extern_ref;
mod features;
mod indexes;
mod initializers;
mod libcalls;
mod memory;
mod module;
mod native;
mod table;
mod trapcode;
mod types;
mod units;
mod utils;
mod values;
mod vmoffsets;

pub use error::{
    CompileError, DeserializeError, ImportError, MiddlewareError, ParseCpuFeatureError,
    PreInstantiationError, SerializeError, WasmError, WasmResult,
};

/// The entity module, with common helpers for Rust structures
pub mod entity;
pub use crate::extern_ref::{ExternRef, VMExternRef};
pub use crate::features::Features;
pub use crate::indexes::{
    CustomSectionIndex, DataIndex, ElemIndex, ExportIndex, FunctionIndex, GlobalIndex, ImportIndex,
    LocalFunctionIndex, LocalGlobalIndex, LocalMemoryIndex, LocalTableIndex, MemoryIndex,
    SignatureIndex, TableIndex,
};
pub use crate::initializers::{
    DataInitializer, DataInitializerLocation, OwnedDataInitializer, TableInitializer,
};
pub use crate::memory::{Memory32, Memory64, MemorySize};
pub use crate::module::{ArchivableModuleInfo, ExportsIterator, ImportsIterator, ModuleInfo};
pub use crate::native::{NativeWasmType, ValueType};
pub use crate::units::{
    Bytes, PageCountOutOfRange, Pages, WASM_MAX_PAGES, WASM_MIN_PAGES, WASM_PAGE_SIZE,
};
pub use crate::values::{Value, WasmValueType};
pub use types::{
    ExportType, ExternType, FunctionType, GlobalInit, GlobalType, ImportType, MemoryType,
    Mutability, TableType, Type, V128,
};

pub use crate::libcalls::LibCall;
pub use crate::memory::MemoryStyle;
pub use crate::table::TableStyle;
pub use crate::trapcode::TrapCode;
pub use crate::vmoffsets::{TargetSharedSignatureIndex, VMBuiltinFunctionIndex, VMOffsets};

pub use crate::utils::is_wasm;

pub use crate::compilation::relocation::{
    Relocation, RelocationKind, RelocationTarget, Relocations,
};
pub use crate::compilation::section::{
    CustomSection, CustomSectionProtection, SectionBody, SectionIndex,
};

pub use crate::compilation::address_map::{FunctionAddressMap, InstructionAddressMap};
pub use crate::compilation::function::{
    Compilation, CompiledFunction, CompiledFunctionFrameInfo, CustomSections, Dwarf, FunctionBody,
    Functions,
};
pub use crate::compilation::module::CompileModuleInfo;
pub use crate::compilation::sourceloc::SourceLoc;
pub use crate::compilation::trap::TrapInformation;
pub use crate::compilation::unwind::CompiledFunctionUnwindInfo;

/// Offset in bytes from the beginning of the function.
pub type CodeOffset = u32;

/// Addend to add to the symbol value.
pub type Addend = i64;

/// Version number of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
