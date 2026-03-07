use inkwell::context::Context;

fn main() {
    let context = Context::create();
    let module = context.create_module("we");
    let builder = context.create_builder();

    // Define `main` function: () -> i32
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[], false);
    let function = module.add_function("main", fn_type, None);
    let basic_block = context.append_basic_block(function, "entry");
    builder.position_at_end(basic_block);
    builder.build_return(Some(&i32_type.const_int(0, false))).unwrap();

    println!("we: LLVM initialized");
    println!("Module:\n{}", module.print_to_string().to_string());
}
