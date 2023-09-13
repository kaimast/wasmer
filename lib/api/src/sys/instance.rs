use crate::errors::InstantiationError;
use crate::exports::Exports;
use crate::module::Module;
use wasmer_vm::{StoreHandle, VMInstance};

use crate::imports::Imports;
use crate::store::AsStoreMut;
use crate::Extern;

#[derive(Clone, PartialEq, Eq)]
pub struct Instance {
    _handle: StoreHandle<VMInstance>,
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

impl From<wasmer_compiler::InstantiationError> for InstantiationError {
    fn from(other: wasmer_compiler::InstantiationError) -> Self {
        match other {
            wasmer_compiler::InstantiationError::Link(e) => Self::Link(e.into()),
            wasmer_compiler::InstantiationError::Start(e) => Self::Start(e.into()),
            wasmer_compiler::InstantiationError::CpuFeature(e) => Self::CpuFeature(e),
        }
    }
}

impl Instance {
    #[allow(clippy::result_large_err)]
    pub(crate) fn new(
        store: &mut impl AsStoreMut,
        module: &Module,
        imports: &Imports,
    ) -> Result<(Self, Exports), InstantiationError> {
        let externs = imports
            .imports_for_module(module)
            .map_err(InstantiationError::Link)?;
        let mut handle = module.0.instantiate(store, &externs)?;
        let exports = Self::get_exports(store, module, &mut handle);

        let instance = Self {
            _handle: StoreHandle::new(store.objects_mut(), handle),
        };

        Ok((instance, exports))
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn new_by_index(
        store: &mut impl AsStoreMut,
        module: &Module,
        externs: &[Extern],
    ) -> Result<(Self, Exports), InstantiationError> {
        let externs = externs.to_vec();
        let mut handle = module.0.instantiate(store, &externs)?;
        let exports = Self::get_exports(store, module, &mut handle);
        let instance = Self {
            _handle: StoreHandle::new(store.objects_mut(), handle),
        };

        Ok((instance, exports))
    }

    fn get_exports(
        store: &mut impl AsStoreMut,
        module: &Module,
        handle: &mut VMInstance,
    ) -> Exports {
        module
            .exports()
            .map(|export| {
                let name = export.name().to_string();
                let export = handle.lookup(&name).expect("export");
                let extern_ = Extern::from_vm_extern(store, export);
                (name, extern_)
            })
            .collect::<Exports>()
    }

    /// Gets the [`Module`] associated with this instance.
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Call a function on a dedicated stack
    /// This allows for async host functions, but may create more overhead
    #[cfg(feature = "async")]
    pub async fn call_with_stack<
        V: Into<crate::Val> + Sized + Send,
        Stack: async_wormhole::stack::Stack + Unpin,
    >(
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
    #[tracing::instrument(skip(imports))]
    pub unsafe fn duplicate(&self, imports: &Imports) -> Result<Self, InstantiationError> {
        let artifact = self.module().artifact();
        let module = self.module().clone();

        let imports = imports
            .imports_for_module(&module)
            .map_err(InstantiationError::Link)?;

        let instance_handle = {
            let old_handle = self.handle.lock().unwrap();
            //FIXME we only need to update the Envs. Do we really need to redo all of this?

            let imports = imports
                .iter()
                .map(crate::Extern::to_export)
                .collect::<Vec<_>>();

            let imports = resolve_imports(
                module.info(),
                imports.as_slice(),
                artifact.finished_dynamic_function_trampolines(),
                artifact.memory_styles(),
                artifact.table_styles(),
            )
            .unwrap();

            old_handle.duplicate(imports, artifact.signatures())
        };

        let exports = self
            .module()
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
            module,
            exports,
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
