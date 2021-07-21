use wasmer::{imports, wat2wasm, Function, Instance, Module, Store};
use wasmer_compiler_llvm::LLVM;
use wasmer_engine_universal::Universal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Let's declare the Wasm module with the text representation.
    let wasm_bytes = wat2wasm(
        br#"
        (module
          (type $no_op_t(func (result i32)))
          (func $no_op_f (type $no_op_t) (result i32)
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

    // Let's instantiate the Wasm module.
    let instance1 = Instance::new(&module, &import_object)?;

    // Create an identical copy of the instance
    let instance2 = instance2.duplicate();


    Ok(())
}

#[test]
fn test_async() -> Result<(), Box<dyn std::error::Error>> {
    main()
}
