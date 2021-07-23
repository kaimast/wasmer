use wasmer::{imports, wat2wasm, WasmerEnv, Function, Instance, Module, Store};
use wasmer_compiler_llvm::LLVM;
use wasmer_engine_universal::Universal;
use std::convert::TryInto;
use std::time::Instant;

#[derive(Clone, Debug, WasmerEnv)]
struct FnEnv {
    number: i32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Let's declare the Wasm module with the text representation.
    let wasm_bytes = wat2wasm(
        br#"
        (module
          (func $host_function (import "" "host_function") (result i32))
          (type $my_func_t(func (result i32)))
          (func $my_func_f (type $my_func_t) (result i32)
           (call $host_function)
          )
          (export "my_func" (func $my_func_f)))
        "#,
    )?;

    // Create a Store.
    let store = Store::new(&Universal::new(LLVM::default()).engine());

    println!("Compiling module...");

    // Let's compile the Wasm module.
    let module = Module::new(&store, wasm_bytes)?;

    fn host_fn(env: &FnEnv) -> i32 {
        println!("HOST {:?}", env);
        env.number
    }

    // Create an import object.
    let host_function1 = Function::new_native_with_env(&store, FnEnv{ number: 42 }, host_fn);
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

    let host_function2 = Function::new_native_with_env(&store, FnEnv{ number: 1337 }, host_fn);
    let import_object2 = imports!{
        "" => {
            "host_function" => host_function2,
        },
    };

    let instance2 = unsafe{ instance1.duplicate(&import_object2) };
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
