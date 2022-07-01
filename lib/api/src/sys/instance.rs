use crate::sys::exports::{Exports, Exportable};
use crate::sys::externals::Extern;
use crate::sys::imports::Imports;
use crate::sys::module::Module;
use crate::sys::store::Store;
use crate::sys::{HostEnvInitError, LinkError, RuntimeError};
use std::fmt;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use wasmer_vm::{InstanceHandle, VMContext};
use wasmer_compiler::resolve_imports;

/// A WebAssembly Instance is a stateful, executable
/// instance of a WebAssembly [`Module`].
///
/// Instance objects contain all the exported WebAssembly
/// functions, memories, tables and globals that allow
/// interacting with WebAssembly.
///
/// Spec: <https://webassembly.github.io/spec/core/exec/runtime.html#module-instances>
#[derive(Clone)]
pub struct Instance {
    handle: Arc<Mutex<InstanceHandle>>,
    module: Module,
    #[allow(dead_code)]
    imports: Vec<Extern>,
    /// The exports for an instance.
    pub exports: Exports,
}

#[cfg(test)]
mod send_test {
    use super::*;

    fn is_send<T: Send>() -> bool {
        true
    }

    #[test]
    fn instance_is_send() {
        assert!(is_send::<Instance>());
    }
}

/// An error while instantiating a module.
///
/// This is not a common WebAssembly error, however
/// we need to differentiate from a `LinkError` (an error
/// that happens while linking, on instantiation), a
/// Trap that occurs when calling the WebAssembly module
/// start function, and an error when initializing the user's
/// host environments.
#[derive(Error, Debug)]
pub enum InstantiationError {
    /// A linking ocurred during instantiation.
    #[error(transparent)]
    Link(LinkError),

    /// A runtime error occured while invoking the start function
    #[error(transparent)]
    Start(RuntimeError),

    /// The module was compiled with a CPU feature that is not available on
    /// the current host.
    #[error("missing requires CPU features: {0:?}")]
    CpuFeature(String),

    /// Error occurred when initializing the host environment.
    #[error(transparent)]
    HostEnvInitialization(HostEnvInitError),
}

impl From<wasmer_compiler::InstantiationError> for InstantiationError {
    fn from(other: wasmer_compiler::InstantiationError) -> Self {
        match other {
            wasmer_compiler::InstantiationError::Link(e) => Self::Link(e),
            wasmer_compiler::InstantiationError::Start(e) => Self::Start(e),
            wasmer_compiler::InstantiationError::CpuFeature(e) => Self::CpuFeature(e),
        }
    }
}

impl From<HostEnvInitError> for InstantiationError {
    fn from(other: HostEnvInitError) -> Self {
        Self::HostEnvInitialization(other)
    }
}

impl Instance {
    /// Creates a new `Instance` from a WebAssembly [`Module`] and a
    /// set of imports using [`Imports`] or the [`imports`] macro helper.
    ///
    /// [`imports`]: crate::imports
    /// [`Imports`]: crate::Imports
    ///
    /// ```
    /// # use wasmer::{imports, Store, Module, Global, Value, Instance};
    /// # fn main() -> anyhow::Result<()> {
    /// let store = Store::default();
    /// let module = Module::new(&store, "(module)")?;
    /// let imports = imports!{
    ///   "host" => {
    ///     "var" => Global::new(&store, Value::I32(2))
    ///   }
    /// };
    /// let instance = Instance::new(&module, &imports)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Errors
    ///
    /// The function can return [`InstantiationError`]s.
    ///
    /// Those are, as defined by the spec:
    ///  * Link errors that happen when plugging the imports into the instance
    ///  * Runtime errors that happen when running the module `start` function.
    pub fn new(module: &Module, imports: &Imports) -> Result<Self, InstantiationError> {
        let store = module.store();
        let imports = imports
            .imports_for_module(module)
            .map_err(InstantiationError::Link)?;
        let handle = module.instantiate(&imports)?;
        let exports = module
            .exports()
            .map(|export| {
                let name = export.name().to_string();
                let export = handle.lookup(&name).expect("export");
                let extern_ = Extern::from_vm_export(store, export.into());
                (name, extern_)
            })
            .collect::<Exports>();

        let instance = Self {
            handle: Arc::new(Mutex::new(handle)),
            module: module.clone(),
            imports,
            exports,
        };

        // # Safety
        // `initialize_host_envs` should be called after instantiation but before
        // returning an `Instance` to the user. We set up the host environments
        // via `WasmerEnv::init_with_instance`.
        //
        // This usage is correct because we pass a valid pointer to `instance` and the
        // correct error type returned by `WasmerEnv::init_with_instance` as a generic
        // parameter.
        unsafe {
            instance
                .handle
                .lock()
                .unwrap()
                .initialize_host_envs::<HostEnvInitError>(&instance as *const _ as *const _)?;
        }

        Ok(instance)
    }

    /// Creates a new `Instance` from a WebAssembly [`Module`] and a
    /// vector of imports.
    ///
    /// ## Errors
    ///
    /// The function can return [`InstantiationError`]s.
    ///
    /// Those are, as defined by the spec:
    ///  * Link errors that happen when plugging the imports into the instance
    ///  * Runtime errors that happen when running the module `start` function.
    pub fn new_by_index(module: &Module, externs: &[Extern]) -> Result<Self, InstantiationError> {
        let store = module.store();
        let imports = externs.to_vec();
        let handle = module.instantiate(&imports)?;
        let exports = module
            .exports()
            .map(|export| {
                let name = export.name().to_string();
                let export = handle.lookup(&name).expect("export");
                let extern_ = Extern::from_vm_export(store, export.into());
                (name, extern_)
            })
            .collect::<Exports>();

        let instance = Self {
            handle: Arc::new(Mutex::new(handle)),
            module: module.clone(),
            imports,
            exports,
        };

        // # Safety
        // `initialize_host_envs` should be called after instantiation but before
        // returning an `Instance` to the user. We set up the host environments
        // via `WasmerEnv::init_with_instance`.
        //
        // This usage is correct because we pass a valid pointer to `instance` and the
        // correct error type returned by `WasmerEnv::init_with_instance` as a generic
        // parameter.
        unsafe {
            instance
                .handle
                .lock()
                .unwrap()
                .initialize_host_envs::<HostEnvInitError>(&instance as *const _ as *const _)?;
        }

        Ok(instance)
    }

    /// Gets the [`Module`] associated with this instance.
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Call a function on a dedicated stack
    /// This allows for async host functions, but may create more overhead
    #[cfg(feature = "async")]
    pub async fn call_with_stack<V: Into<crate::Val>+Sized+Send, Stack: async_wormhole::stack::Stack + Unpin>(
        &self,
        func_name: &str,
        stack: Stack,
        mut params: Vec<V>,
    ) -> (Result<Box<[crate::Val]>, RuntimeError>, Stack) {
        use std::iter::FromIterator;

        let mut task = async_wormhole::AsyncWormhole::new(
            stack,
            |yielder| -> Result<Box<[crate::Val]>, RuntimeError> {
                // Make sure the yielder does not get moved around in memory by pinning it
                let yielder = Box::pin(yielder);
                let yielder_ptr: *mut std::ffi::c_void = unsafe { std::mem::transmute(&*yielder) };

                let func = self
                    .exports
                    .get_function(func_name)
                    .expect("No such function");
                {
                    let hdl = self.handle.lock().unwrap();
                    hdl.set_yielder(yielder_ptr);
                }

                let params = Vec::from_iter(params.drain(..).map(|p| p.into()));
                func.call(&params)
            },
        )
        .expect("Failed to create async function call");

        {
            use wasmer_vm::TlsRestore;
            let tls_store: Mutex<(bool, Option<TlsRestore>)> = Mutex::new((false, None));

            // This mirrors code from lunatic
            // See https://github.com/lunatic-solutions/lunatic/blob/5ba519e2421d6531266955201f86e641d8c777ec/src/api/process/tls.rs#L14
            task.set_pre_post_poll(move || {
                let mut tls_store = tls_store.lock().unwrap();
                let (init, tls_restore) = &mut *tls_store;

                // On the first poll there is nothing to preserve yet
                if *init {
                    if let Some(tls) = tls_restore.take() {
                        unsafe { tls.replace() }.expect("Failed to restore TLS");
                    } else {
                        let tls = unsafe { TlsRestore::take() }.expect("Failed to store TLS");
                        *tls_restore = Some(tls);
                    }
                } else {
                    *init = true;
                }
            });
        }

        let result = (&mut task).await;

        (result, task.stack())
    }

    /// Returns the [`Store`] where the `Instance` belongs.
    pub fn store(&self) -> &Store {
        self.module.store()
    }

    #[doc(hidden)]
    pub fn vmctx_ptr(&self) -> *mut VMContext {
        self.handle.lock().unwrap().vmctx_ptr()
    }

    /// Duplicate the entire state of this instance and create a new one
    #[ tracing::instrument(skip(imports)) ]
    pub unsafe fn duplicate(&self, imports: &Imports) -> Result<Self, InstantiationError> {
        let artifact = self.module().artifact();
        let module = self.module().clone();

        let imports = imports
            .imports_for_module(&module)
            .map_err(InstantiationError::Link)?;

        let instance_handle = {
            let old_handle = self.handle.lock().unwrap();
            //FIXME we only need to update the Envs. Do we really need to redo all of this?

            let imports = imports.iter()
                .map(crate::Extern::to_export)
                .collect::<Vec<_>>();

            let imports = resolve_imports(module.info(),
                imports.as_slice(),
                artifact.finished_dynamic_function_trampolines(),
                artifact.memory_styles(), artifact.table_styles()
            ).unwrap();

            old_handle.duplicate(imports, artifact.signatures())
        };

        let exports = self.module()
            .exports()
            .map(|export| {
                let name = export.name().to_string();
                let export = instance_handle.lookup(&name).expect("export");
                let extern_ = Extern::from_vm_export(self.store(), export.into());
                (name, extern_)
            })
            .collect::<Exports>();

        let instance = Self {
            handle: Arc::new(Mutex::new(instance_handle)),
            module, exports,
            imports,
        };

        {
            let mut hdl = instance.handle.lock().unwrap();
            hdl.initialize_host_envs::<HostEnvInitError>(&instance as *const _ as *const _)?;

            let data_initializers = artifact
                .data_initializers()
                .iter()
                .map(|init| wasmer_types::DataInitializer {
                    location: init.location.clone(),
                    data: &*init.data,
                })
                .collect::<Vec<_>>();

            hdl.finish_duplication(&data_initializers);
        }

        Ok(instance)
    }
}

impl fmt::Debug for Instance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Instance")
            .field("exports", &self.exports)
            .finish()
    }
}
