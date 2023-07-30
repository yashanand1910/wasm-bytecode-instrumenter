use walrus::{
    ir::{
        BinaryOp, Binop, Const, Instr, InstrSeqId, Load, LoadKind, MemArg, Store, StoreKind, Value,
    },
    ExportItem, LocalFunction, MemoryId, Module,
};

// Size in bytes for storing a count
const SIZE: usize = 8;

// Struct to store info on insertion locations for an instruction sequence.
// Note that blocks can be indefinitely nested.
#[derive(Debug)]
struct ProbeInsertLocs {
    id: InstrSeqId,
    positions: Vec<(usize, Option<ProbeInsertLocs>)>,
}

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
    let mem_id = module.memories.add_local(false, 1, None);
    module.exports.add("mem", ExportItem::Memory(mem_id));

    // Iterate on local functions
    let mut foffsets: Vec<usize> = Vec::new();
    let mut curr_foffset = 0;
    module.funcs.iter_local_mut().for_each(|(_, func)| {
        // Add function offset
        foffsets.push(curr_foffset);

        curr_foffset = instrument_func(func, curr_foffset, mem_id);
    });

    module
}

/// Instrument a local function and return size (in bytes)
/// of memory it will require to capture its instrumentation data
/// FIXME: Offset locations are incorrect
fn instrument_func(func: &mut LocalFunction, foffset: usize, mem_id: MemoryId) -> usize {
    // Get insert locations for probe insertion
    let probe_insert_locs = get_probe_insert_locs(func, func.entry_block());

    // Insert probes (counting instructions) at the locations
    let insert_count = insert_probes(func, &probe_insert_locs, &foffset, &mem_id);

    insert_count * SIZE
}

fn get_probe_insert_locs(func: &LocalFunction, instr_seq_id: InstrSeqId) -> ProbeInsertLocs {
    let mut insert_locs = ProbeInsertLocs {
        id: instr_seq_id,
        positions: vec![],
    };

    func.block(instr_seq_id)
        .iter()
        .enumerate()
        .for_each(|(i, (instr, _))| {
            // Recurse for nexted blocks
            match instr {
                Instr::Block(block) => {
                    let block_insert_locs = get_probe_insert_locs(func, block.seq);
                    insert_locs.positions.push((i, Some(block_insert_locs)));
                }
                Instr::Loop(block) => {
                    let block_insert_locs = get_probe_insert_locs(func, block.seq);
                    insert_locs.positions.push((i, Some(block_insert_locs)));
                }
                Instr::IfElse(block) => {
                    let if_block_insert_locs = get_probe_insert_locs(func, block.consequent);
                    let else_block_insert_locs = get_probe_insert_locs(func, block.alternative);
                    insert_locs.positions.push((i, Some(if_block_insert_locs)));
                    insert_locs
                        .positions
                        .push((i, Some(else_block_insert_locs)));
                }
                Instr::BrIf(_) | Instr::BrTable(_) => {
                    insert_locs.positions.push((i, None));
                }
                _ => {}
            }
        });

    insert_locs
}

/// Insert probes at the provided insert locations
/// Recursively does it for all nested blocks and returns
/// total count of inserted probes
/// FIXME: Count based on top operand value
fn insert_probes(
    func: &mut LocalFunction,
    insert_locs: &ProbeInsertLocs,
    foffset: &usize,
    mem_id: &MemoryId,
) -> usize {
    let mut inserts_so_far: usize = 0;
    let mut probe_count = 0;
    for (pos_orig, block_insert_locs_option) in &insert_locs.positions {
        match block_insert_locs_option {
            Some(block_insert_locs) => {
                probe_count += insert_probes(func, block_insert_locs, foffset, mem_id);
            }
            _ => {}
        }
        let func_builder = func.builder_mut();
        let mut instr_builder = func_builder.instr_seq(insert_locs.id);

        let ioffset: i32 = (foffset + pos_orig) as i32;
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
                memory: *mem_id,
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
                memory: *mem_id,
                kind: StoreKind::I64 { atomic: false },
                arg: MemArg {
                    align: 0, // XXX: Not sure if alignment is OK
                    offset: 0,
                },
            }),
        );
        i += 1;

        inserts_so_far = i - pos_orig;
        probe_count += 1;
    }

    probe_count
}
