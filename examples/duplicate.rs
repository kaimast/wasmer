use wasmer::{imports, wat2wasm, Instance, Module, Store};
use wasmer_compiler_llvm::LLVM;
use wasmer_engine_universal::Universal;
use std::convert::TryInto;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Let's declare the Wasm module with the text representation.
    let wasm_bytes = wat2wasm(
        br#"
        (module
          (type $no_op_t(func (result i32)))
          (func $no_op_f (type $no_op_t) (result i32)
            i32.const 1337
          )
          (export "no_op" (func $no_op_f)))
        "#,
    )?;

    // Create a Store.
    let store = Store::new(&Universal::new(LLVM::default()).engine());

    println!("Compiling module...");

    // Let's compile the Wasm module.
    let module = Module::new(&store, wasm_bytes)?;

    // Create an import object.
    let import_object = imports!{};
    println!("Instantiating module...");

    let start = Instant::now();

    // Let's instantiate the Wasm module.
    let instance1 = Instance::new(&module, &import_object)?;

    let end = Instant::now();
    println!("Took {}us", (end-start).as_micros());

    println!("Cloning instance");
    let start = Instant::now();

    // Create an identical copy of the instance
    let instance2 = instance1.duplicate();
    let end = Instant::now();
    println!("Took {}us", (end-start).as_micros());


    let func1 = instance1.exports.get_function("no_op").unwrap();
    let func2 = instance2.exports.get_function("no_op").unwrap();

    let result1 = func1.call(&[]).expect("Function call 1 failed");
    let result2 = func2.call(&[]).expect("Function call 2 failed");

    let result1: i32 = result1[0].clone().try_into().unwrap();
    let result2: i32 = result2[0].clone().try_into().unwrap();

    assert!(result1 == 1337);
    assert!(result2 == 1337);

    println!("Success");

    Ok(())
}

#[test]
fn test_async() -> Result<(), Box<dyn std::error::Error>> {
    main()
}
