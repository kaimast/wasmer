use wasmer::{imports, wat2wasm, Function, Instance, LazyInit, Module, Store, WasmerEnv, Yielder};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_jit::JIT;

use async_wormhole::stack::{EightMbStack, Stack};

#[derive(Clone, Default, WasmerEnv)]
struct AsyncEnv {
    #[wasmer(yielder)]
    yielder: LazyInit<Yielder>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Let's declare the Wasm module with the text representation.
    let wasm_bytes = wat2wasm(
        br#"
        (module
          (func $my_async_fn (import "env" "my_async_fn") (param i32) (result i32))

          (type $call_func_t (func (result i32)))
          (func $call_func_f (type $call_func_t) (result i32)
            (call $my_async_fn(i32.const 3))
          )
          (export "call_func" (func $call_func_f)))
        "#,
    )?;

    // Create a Store.
    // Note that we don't need to specify the engine/compiler if we want to use
    // the default provided by Wasmer.
    // You can use `Store::default()` for that.
    let store = Store::new(&JIT::new(Cranelift::default()).engine());

    println!("Compiling module...");
    // Let's compile the Wasm module.
    let module = Module::new(&store, wasm_bytes)?;

    // Create the function
    fn my_async_fn(env: &AsyncEnv, a: i32) -> i32 {
        println!("Calling `my_async_fn`...");
        let yielder = env.yielder.get_ref().unwrap().get();

        let result = yielder.async_suspend(async move { 52 * a });

        println!("Result of `my_async_fn`: {:?}", result);

        result
    }

    let async_env = AsyncEnv::default();
    let my_async_fn = Function::new_native_with_env(&store, async_env, my_async_fn);

    // Create an import object.
    let import_object = imports! {
        "env" => {
            "my_async_fn" => my_async_fn,
        }
    };

    println!("Instantiating module...");

    // Let's instantiate the Wasm module.
    let instance = Instance::new(&module, &import_object)?;
    let stack = EightMbStack::new()?;

    let result = smol::block_on(async move { instance.call_with_stack("call_func", stack).await })?;

    let result = result[0].unwrap_i32();
    println!("Results of `call_func`: {:?}", result);
    assert_eq!(result, 156);

    Ok(())
}

#[test]
fn test_async() -> Result<(), Box<dyn std::error::Error>> {
    main()
}
