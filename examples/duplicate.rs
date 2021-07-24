use wasmer::{imports, wat2wasm, WasmerEnv, Function, Instance, Module, Store, LazyInit, Memory};
use wasmer_compiler_llvm::LLVM;
use wasmer_engine_universal::Universal;
use wasmer_engine::Engine;

use std::convert::TryInto;
use std::time::Instant;

#[derive(Clone, Debug, WasmerEnv)]
struct FnEnv {
    #[ wasmer(export) ]
    memory: LazyInit<Memory>,
    number: i32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Let's declare the Wasm module with the text representation.
    let wasm_bytes = wat2wasm(
        br#"
        (module
          (func $host_function (import "" "host_function") (result i32))
          (memory (export "memory") 1)

          (type $my_func_t(func (result i32)))
          (func $my_func_f (type $my_func_t) (result i32)
           (call $host_function)
          )
          (export "my_func" (func $my_func_f)))
        "#,
    )?;

    let engine = Universal::new(LLVM::default()).engine();
    let mut tunables = wasmer::BaseTunables::for_target(engine.target());
    tunables.static_memory_bound = wasmer::Pages(0); // Always use dynamic memory

    // Create a Store.
    let store = Store::new_with_tunables(&engine, tunables);

    println!("Compiling module...");

    // Let's compile the Wasm module.
    let module = Module::new(&store, wasm_bytes)?;

    fn host_fn(env: &FnEnv) -> i32 {
        if env.memory.get_ref().is_none() {
            panic!("Memory not initialized");
        }

        env.number
    }

    // Create an import object.
    let env1 = FnEnv{ number: 42, memory: Default::default() };
    let host_function1 = Function::new_native_with_env(&store, env1, host_fn);
    let import_object1 = imports!{
        "" => {
            "host_function" => host_function1,
        },
    };

    println!("Instantiating module...");

    let start = Instant::now();

    // Let's instantiate the Wasm module.
    let instance1 = Instance::new(&module, &import_object1)?;

    let end = Instant::now();
    println!("Took {}us", (end-start).as_micros());

    println!("Cloning instance");
    let start = Instant::now();

    let env2 = FnEnv{ number: 1337, memory: Default::default() };
    let host_function2 = Function::new_native_with_env(&store, env2, host_fn);
    let import_object2 = imports!{
        "" => {
            "host_function" => host_function2,
        },
    };

    let instance2 = unsafe{ instance1.duplicate(&import_object2).expect("Duplication failed") };
    let end = Instant::now();
    println!("Took {}us", (end-start).as_micros());

    let func1 = instance1.exports.get_function("my_func").unwrap();
    let func2 = instance2.exports.get_function("my_func").unwrap();

    let result1 = func1.call(&[]).expect("Function call 1 failed");
    let result2 = func2.call(&[]).expect("Function call 2 failed");

    let result1: i32 = result1[0].clone().try_into().unwrap();
    let result2: i32 = result2[0].clone().try_into().unwrap();

    assert!(result1 == 42);
    assert!(result2 == 1337);

    println!("Success");

    Ok(())
}

#[test]
fn test_async() -> Result<(), Box<dyn std::error::Error>> {
    main()
}
