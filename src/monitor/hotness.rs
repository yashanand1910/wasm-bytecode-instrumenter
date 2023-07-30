use walrus::{
    ir::{BinaryOp, Binop, Const, Instr, Load, LoadKind, MemArg, Store, StoreKind, Value},
    ExportItem, Module,
};

// Size in bytes for storing a count
const SIZE: usize = 8;

/// Adds hotness instrumentation logic to a module.
///     1.  Add a linear memory to keep track of counts
///     2.  If a local function has `n` instructions `n * SIZE` bytes will
///         be reserved for storing counts for each instruction.
///     3.  Maintain starting offset `foffset` in the memory segment for each
///         function se we can calculate the memory offset for an
///         instruction as `foffset + ioffset` to increment count.
///     4.  TODO: Figure out how to print the output at the end.
pub fn instrument(mut module: Module) -> Module {
    // Add linear memory for storing counts
    // XXX: Might need to adjust max size
    // XXX: Might need to initialize to 0
    let mem_id = module.memories.add_local(false, 1, Some(1));
    // Export memory
    module.exports.add("mem", ExportItem::Memory(mem_id));

    // Local function offsets list
    let mut foffsets: Vec<usize> = Vec::new();
    let mut curr_foffset = 0;

    // Iterate on local functions
    module.funcs.iter_local_mut().for_each(|(_func_id, func)| {
        let func_builder = func.builder_mut();
        let mut instr_builder = func_builder.func_body();
        let instr_seq = instr_builder.instrs_mut();

        // Get positions in function where to insert instructions
        let mut insert_positions: Vec<usize> = Vec::new();
        instr_seq.iter().enumerate().for_each(|(i, (instr, _))| {
            // TODO: Deal with nested blocks
            match instr {
                _ => {
                    insert_positions.push(i);
                }
            }
        });

        // Insert counting instructions at the captured positions.
        // The positions will need to be offseted by the number of new
        // instructions that have been inserted.
        let mut inserts_so_far: usize = 0;
        for pos_orig in insert_positions.iter() {
            let ioffset: i32 = (curr_foffset + pos_orig) as i32;
            let mut i = pos_orig + inserts_so_far;

            // Insert store index const instr
            let store_index = Const {
                value: Value::I32(ioffset),
            };
            let i64_const_store_index: Instr = Instr::Const(store_index);
            instr_builder.instr_at(i, i64_const_store_index);
            i += 1;

            // Insert load index const instr
            let load_index = Const {
                value: Value::I32(ioffset),
            };
            let i64_const_load_index: Instr = Instr::Const(load_index);
            instr_builder.instr_at(i, i64_const_load_index);
            i += 1;

            // Insert load instr
            instr_builder.instr_at(
                i,
                Instr::Load(Load {
                    memory: mem_id,
                    kind: LoadKind::I64 { atomic: false },
                    arg: MemArg {
                        align: 0, // XXX: Not sure if alignment is OK
                        offset: 0,
                    },
                }),
            );
            i += 1;

            // Insert increment count const
            let incr_count = Const {
                value: Value::I64(1),
            };
            let i64_const_incr_count: Instr = Instr::Const(incr_count);
            instr_builder.instr_at(i, i64_const_incr_count);
            i += 1;

            // Insert add instr
            instr_builder.instr_at(
                i,
                Instr::Binop(Binop {
                    op: BinaryOp::I64Add,
                }),
            );
            i += 1;

            // Insert store instr
            instr_builder.instr_at(
                i,
                Instr::Store(Store {
                    memory: mem_id,
                    kind: StoreKind::I64 { atomic: false },
                    arg: MemArg {
                        align: 0, // XXX: Not sure if alignment is OK
                        offset: 0,
                    },
                }),
            );
            i += 1;

            inserts_so_far = i - pos_orig;
        }

        // Add function offset
        foffsets.push(curr_foffset);
        curr_foffset = insert_positions.len() * SIZE;
    });

    module
}
