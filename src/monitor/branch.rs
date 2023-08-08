use std::cmp::max;

use walrus::{
    ir::{
        BinaryOp, Binop, Const, Instr, InstrSeqId, Load, LoadKind, MemArg, Store, StoreKind, Value,
    },
    ExportItem, LocalFunction, LocalId, Memory, MemoryId, Module, ModuleTypes, ValType,
};

use crate::monitor::MEMUNIT;

use super::{COUNTSIZE, MEMREGION};

// Struct to store info on insertion locations for an instruction sequence.
// Note that blocks can be indefinitely nested.
#[derive(Debug)]
struct ProbeInsertLocs {
    id: InstrSeqId,

    // (instr position, no. of paths, nested ProbeInsertLocs)
    positions: Vec<(usize, usize, Option<ProbeInsertLocs>)>,
}

/// Adds branch instrumentation logic to a module.
///     1.  Add a linear memory to keep track of counts
///     2.  If a local function has `n` instructions `n * SIZE` bytes will
///         be reserved for storing counts for each instruction.
///     3.  Maintain starting offset `foffset` in the memory segment for each
///         function se we can calculate the memory offset for an
///         instruction as `foffset + ioffset` to increment count.
///     4.  For each branching instr copies top of stack into a local.
///         Then uses the top of stack in an if condition to increment
///         count in memory and then restores the top of stack from local.
///     5.  TODO: Figure out how to print the output at the end.
pub fn instrument(mut module: Module) -> Module {
    // Add linear memory for storing counts
    // XXX: Might need to initialize to 0
    let mem_id = module.memories.add_local(false, 1, None);
    module.exports.add(MEMREGION, ExportItem::Memory(mem_id));

    // Create local var to save top of stack
    let local_id = module.locals.add(ValType::I32);

    // Iterate on local functions
    let mut foffsets: Vec<usize> = Vec::new();
    let mut curr_foffset = 0;
    module.funcs.iter_local_mut().for_each(|(_, func)| {
        // Add function offset
        foffsets.push(curr_foffset);

        curr_foffset += instrument_func(&mut module.types, func, curr_foffset, mem_id, local_id);
    });

    // Update size of memory region
    let mem_region: &mut Memory = module.memories.get_mut(mem_id);
    let mem_size = (max(1, curr_foffset / MEMUNIT)) as u32;
    mem_region.initial = mem_size;
    mem_region.maximum = Some(mem_size);

    module
}

/// Instrument a local function and return size (in bytes)
/// of memory it will require to capture its instrumentation data
fn instrument_func(
    types: &mut ModuleTypes,
    func: &mut LocalFunction,
    foffset: usize,
    mem_id: MemoryId,
    local_id: LocalId,
) -> usize {
    // Get insert locations for probe insertion
    let probe_insert_locs = get_probe_insert_locs(func, func.entry_block());

    // println!("{:#?}", probe_insert_locs);

    // Insert probes (counting instructions) at the locations
    let insert_count = insert_probes(
        types,
        func,
        &probe_insert_locs,
        &foffset,
        &mem_id,
        &local_id,
    );

    insert_count * COUNTSIZE
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
                    insert_locs.positions.push((i, 0, Some(block_insert_locs)));
                }
                Instr::Loop(block) => {
                    let block_insert_locs = get_probe_insert_locs(func, block.seq);
                    insert_locs.positions.push((i, 0, Some(block_insert_locs)));
                }
                Instr::IfElse(block) => {
                    let if_block_insert_locs = get_probe_insert_locs(func, block.consequent);
                    let else_block_insert_locs = get_probe_insert_locs(func, block.alternative);
                    insert_locs
                        .positions
                        .push((i, 0, Some(if_block_insert_locs)));
                    insert_locs
                        .positions
                        .push((i, 0, Some(else_block_insert_locs)));

                    // We also need to evaluate the operand before the if block
                    insert_locs.positions.push((i, 2, None));
                }
                Instr::BrTable(table) => {
                    let table_blocks: &[InstrSeqId] = &table.blocks[..];
                    for &block in table_blocks {
                        let block_insert_locs = get_probe_insert_locs(func, block);
                        insert_locs.positions.push((i, 0, Some(block_insert_locs)));
                    }

                    let default_block_insert_locs = get_probe_insert_locs(func, table.default);
                    insert_locs
                        .positions
                        .push((i, 0, Some(default_block_insert_locs)));

                    // We also need to evaluate the operand before the br_table instr
                    insert_locs
                        .positions
                        .push((i, table_blocks.len() + 1, None));
                }
                Instr::BrIf(_) => {
                    insert_locs.positions.push((i, 2, None));
                }
                Instr::Call(_)
                | Instr::CallIndirect(_)
                | Instr::LocalGet(_)
                | Instr::LocalSet(_)
                | Instr::LocalTee(_)
                | Instr::GlobalGet(_)
                | Instr::GlobalSet(_)
                | Instr::Const(_)
                | Instr::Binop(_)
                | Instr::Unop(_)
                | Instr::Select(_)
                | Instr::Unreachable(_)
                | Instr::Br(_)
                | Instr::Drop(_)
                | Instr::Return(_)
                | Instr::MemorySize(_)
                | Instr::MemoryGrow(_)
                | Instr::MemoryInit(_)
                | Instr::DataDrop(_)
                | Instr::MemoryCopy(_)
                | Instr::MemoryFill(_)
                | Instr::Load(_)
                | Instr::Store(_)
                | Instr::AtomicRmw(_)
                | Instr::Cmpxchg(_)
                | Instr::AtomicNotify(_)
                | Instr::AtomicWait(_)
                | Instr::AtomicFence(_)
                | Instr::TableGet(_)
                | Instr::TableSet(_)
                | Instr::TableGrow(_)
                | Instr::TableSize(_)
                | Instr::TableFill(_)
                | Instr::RefNull(_)
                | Instr::RefIsNull(_)
                | Instr::RefFunc(_)
                | Instr::V128Bitselect(_)
                | Instr::I8x16Swizzle(_)
                | Instr::I8x16Shuffle(_)
                | Instr::LoadSimd(_)
                | Instr::TableInit(_)
                | Instr::ElemDrop(_)
                | Instr::TableCopy(_) => {
                    // do nothing
                }
            }
        });

    insert_locs
}

/// Insert probes at the provided insert locations
/// Recursively does it for all nested blocks and returns
/// total count of inserted probes
fn insert_probes(
    types: &mut ModuleTypes,
    func: &mut LocalFunction,
    insert_locs: &ProbeInsertLocs,
    foffset: &usize,
    mem_id: &MemoryId,
    local_id: &LocalId,
) -> usize {
    let mut inserts_so_far: usize = 0;
    let mut probe_count = 0;
    for (_, (pos_orig, npaths, block_insert_locs_option)) in
        insert_locs.positions.iter().enumerate()
    {
        let ioffset = foffset + (probe_count * COUNTSIZE); // offset for storing count

        match block_insert_locs_option {
            Some(block_insert_locs) => {
                let insert_count =
                    insert_probes(types, func, block_insert_locs, &ioffset, mem_id, local_id);
                probe_count += insert_count;
            }
            None => {
                let func_builder = func.builder_mut();
                let mut instr_builder = func_builder.instr_seq(insert_locs.id);
                let mut i = pos_orig + inserts_so_far;

                // Save top of stack to local var
                instr_builder.local_tee_at(i, *local_id);
                i += 1;

                // Restore top of stack from local var
                instr_builder.local_get_at(i, *local_id);
                i += 1;

                // If 2 paths then set load/store index using if block
                // TODO: Treat br_table specially (for experiment its fine
                // as even for br table it can be a single if else block)
                let ty = types.add(&[], &[ValType::I32, ValType::I32]);
                instr_builder.if_else_at(
                    i,
                    ty,
                    // Need two return 2 consts: one for load and one for store
                    |then| {
                        then.i32_const(ioffset as i32).i32_const(ioffset as i32);
                    },
                    |else_| {
                        else_
                            .i32_const((ioffset + COUNTSIZE) as i32)
                            .i32_const((ioffset + COUNTSIZE) as i32);
                    },
                );
                i += 1;

                // Insert load instr
                instr_builder.instr_at(
                    i,
                    Instr::Load(Load {
                        memory: *mem_id,
                        kind: LoadKind::I32 { atomic: false },
                        arg: MemArg {
                            align: COUNTSIZE as u32,
                            offset: 0,
                        },
                    }),
                );
                i += 1;

                // Insert increment count const
                let incr_count = Const {
                    value: Value::I32(1),
                };
                let i32_const_incr_count: Instr = Instr::Const(incr_count);
                instr_builder.instr_at(i, i32_const_incr_count);
                i += 1;

                // Insert add instr
                instr_builder.instr_at(
                    i,
                    Instr::Binop(Binop {
                        op: BinaryOp::I32Add,
                    }),
                );
                i += 1;

                // Insert store instr
                instr_builder.instr_at(
                    i,
                    Instr::Store(Store {
                        memory: *mem_id,
                        kind: StoreKind::I32 { atomic: false },
                        arg: MemArg {
                            align: COUNTSIZE as u32,
                            offset: 0,
                        },
                    }),
                );
                i += 1;

                inserts_so_far = i - pos_orig;
                probe_count += npaths;
            }
        }
    }

    probe_count
}
