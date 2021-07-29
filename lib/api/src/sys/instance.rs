use crate::sys::exports::Exports;
use crate::sys::externals::Extern;
use crate::sys::module::Module;
use crate::sys::store::Store;
use crate::sys::{HostEnvInitError, LinkError, RuntimeError};
use loupe::MemoryUsage;
use std::fmt;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use wasmer_engine::Resolver;
use wasmer_vm::{InstanceHandle, VMContext};

/// A WebAssembly Instance is a stateful, executable
/// instance of a WebAssembly [`Module`].
///
/// Instance objects contain all the exported WebAssembly
/// functions, memories, tables and globals that allow
/// interacting with WebAssembly.
///
/// Spec: <https://webassembly.github.io/spec/core/exec/runtime.html#module-instances>
#[derive(Clone, MemoryUsage)]
pub struct Instance {
    handle: Arc<Mutex<InstanceHandle>>,
    module: Module,
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

    /// Error occurred when initializing the host environment.
    #[error(transparent)]
    HostEnvInitialization(HostEnvInitError),
}

impl From<wasmer_engine::InstantiationError> for InstantiationError {
    fn from(other: wasmer_engine::InstantiationError) -> Self {
        match other {
            wasmer_engine::InstantiationError::Link(e) => Self::Link(e),
            wasmer_engine::InstantiationError::Start(e) => Self::Start(e),
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
    /// set of imports resolved by the [`Resolver`].
    ///
    /// The resolver can be anything that implements the [`Resolver`] trait,
    /// so you can plug custom resolution for the imports, if you wish not
    /// to use [`ImportObject`].
    ///
    /// The [`ImportObject`] is the easiest way to provide imports to the instance.
    ///
    /// [`ImportObject`]: crate::ImportObject
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
    #[ tracing::instrument(skip(resolver)) ]
    pub fn new(module: &Module, resolver: &dyn Resolver) -> Result<Self, InstantiationError> {
        let store = module.store();
        let handle = module.instantiate(resolver)?;

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

    ///TODO
    #[cfg(feature = "async")]
    pub async fn call_with_stack<Stack: async_wormhole::stack::Stack + Unpin>(
        &self,
        func_name: &str,
        stack: Stack,
        //FIXME support passing arguments params: &[Val]
    ) -> Result<Box<[crate::Val]>, RuntimeError> {
        let mut task = async_wormhole::AsyncWormhole::new(
            stack,
            |yielder| -> Result<Box<[crate::Val]>, RuntimeError> {
                let yielder_ptr: *mut std::ffi::c_void = unsafe { std::mem::transmute(&yielder) };

                let func = self
                    .exports
                    .get_function(func_name)
                    .expect("No such function");
                {
                    let hdl = self.handle.lock().unwrap();
                    hdl.set_yielder(yielder_ptr);
                }

                let params = &[];
                func.call(params)
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

        task.await
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
    #[ tracing::instrument(skip(resolver)) ]
    pub unsafe fn duplicate(&self, resolver: &dyn Resolver) -> Result<Self, InstantiationError> {
        let handle = self.handle.lock().unwrap();
        let artifact = self.module().artifact();
        let module = self.module();

        //FIXME we only need to update the Envs. do we really need to redo all of this?
        let imports = wasmer_engine::resolve_imports(module.info(), resolver, artifact.finished_dynamic_function_trampolines(), artifact.memory_styles(), artifact.table_styles()).unwrap();

        let instance_handle = handle.duplicate(imports, artifact.signatures(), artifact.func_data_registry());

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
            module: self.module.clone(), exports,
        };

        instance
            .handle
            .lock().unwrap()
            .initialize_host_envs::<HostEnvInitError>(&instance as *const _ as *const _)?;

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
