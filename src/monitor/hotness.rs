use std::path::Path;

use walrus::{FunctionBuilder, Module};

/// Adds hotness instrumentation bytecode to a module
pub fn instrument(mut module: Module, path: &Path) -> Module {
    // Add memory region for storing counts
    let mem_id = module.memories.add_local(false, 1, Some(1));

    for (func_id, func) in module.funcs.iter_local_mut() {
        let func_builder = func.builder_mut();
        let mut instr_builder = func_builder.func_body();
        let instr_seq = instr_builder.instrs_mut();

        // Get all positions in function to insert counting instructions
        let mut probe_positions: Vec<usize> = Vec::new();
        for (i, (instr, _)) in instr_seq.iter().enumerate() {
            // TODO: Deal with nested blocks
            match instr {
                _ => {
                    probe_positions.push(i);
                }
            }
        }

        // Add memory to store count
        let probe_func = FunctionBuilder::new(&mut module.types, &vec![], &vec![]);

        // Insert all probes at the captured positions
        for pos in probe_positions {
            // instr_builder.call_at(pos, probe_func.func_body_id());
        }
    }

    module
}
