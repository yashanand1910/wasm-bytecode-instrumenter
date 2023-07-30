use std::path::{Path, PathBuf};

use walrus::{ir::Instr, FunctionBuilder, LocalId, Module};

pub enum Monitor {
    Branch,
    Hotness,
}

impl Monitor {
    fn to_str(&self) -> &str {
        match self {
            Monitor::Branch => "branch",
            Monitor::Hotness => "hotness",
        }
    }
}

/// Adds monitor instrumentation bytecode to an existing
/// WASM module.
pub fn add_monitor(module: Module, monitor: Monitor, path: &Path) -> walrus::Result<()> {
    let mut clone_module = match Module::from_file(path) {
        Ok(module) => module,
        _ => panic!("unreachable"),
    };

    for (func_id, func) in module.funcs.iter_local() {
        let type_id = func.ty();
        let func_params = module.types.params(type_id);
        let func_results = module.types.results(type_id);
        let mut clone_func_builder =
            FunctionBuilder::new(&mut clone_module.types, func_params, func_results);
        let mut clone_instr_builder = clone_func_builder.func_body();

        // XXX: Re-add arg locals
        // for local in module.locals.iter() {
        //     clone_module.locals.add(local.clone().ty());
        // }

        let n = clone_module.locals.iter().next().unwrap().id();

        // Recreate function block
        let instr_seq_iq = func.entry_block();
        let instr_seq = func.block(instr_seq_iq);
        for (instr, _instr_id) in &instr_seq.instrs {
            match instr {
                // Instr::Block(_) => {
                //     clone_instr_builder.instr(instr.clone());
                // }
                _ => {
                    clone_instr_builder.instr(instr.clone());
                    // clone_instr_builder.i32_const(877);
                    // clone_instr_builder.local_set(n);
                }
            };
        }

        clone_func_builder.finish(vec![n], &mut clone_module.funcs);

        // let mut instr_builder = func.builder().func_body();
        // for (instr, instr_id) in instr_builder.instrs_mut() {
        //     match instr {
        //         _ => println!("id: {:?}", instr),
        //     }
        // }
        // for (instr, instr_id) in instr_builder.instr_seq() {
        //     match instr {
        //         Instr::Block(block) => match instr_builder.instr_seq(block.seq) {
        //             _ => println!("id: {:?}", instr),
        //         },
        //         _ => println!("id: {:?}", instr),
        //     }
        // }
    }

    write_module(clone_module, &monitor, path)
}

/// Writes the WASM module to the given path adding
/// branch name to the file name.
fn write_module(mut module: Module, monitor: &Monitor, path: &Path) -> walrus::Result<()> {
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let extension = path.extension().unwrap().to_str().unwrap();
    let new_file_stem = format!("{}-{}", file_stem, monitor.to_str());
    let new_file_name = PathBuf::from(format!("{}.{}", new_file_stem, extension));

    module.emit_wasm_file(path.with_file_name(new_file_name))
}
