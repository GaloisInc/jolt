use itertools::Itertools;
use rayon::prelude::*;
use tracer::{instruction::RV32IMCycle, LazyTraceIterator};

use crate::{
    field::JoltField,
    jolt::vm::{instruction_lookups, ram::remap_address, JoltProverPreprocessing},
    poly::{
        commitment::commitment_scheme::CommitmentScheme, compact_polynomial::StreamingCompactWitness, multilinear_polynomial::{MultilinearPolynomial, StreamingWitness}, one_hot_polynomial::{OneHotPolynomial, StreamingOneHotPolynomial, StreamingOneHotWitness}
    },
};

use super::instruction::{CircuitFlags, InstructionFlags, LookupQuery};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommittedPolynomials {
    /* R1CS aux variables */
    /// The "left" input to the current instruction. Typically either the
    /// rs1 value or the current program counter.
    LeftInstructionInput,
    /// The "right" input to the current instruction. Typically either the
    /// rs2 value or the immediate value.
    RightInstructionInput,
    /// Product of `LeftInstructionInput` and `RightInstructionInput`
    Product,
    /// Whether the current instruction should write the lookup output to
    /// the destination register
    WriteLookupOutputToRD,
    /// Whether the current instruction should write the program counter to
    /// the destination register
    WritePCtoRD,
    /// Whether the current instruction triggers a branch
    ShouldBranch,
    /// Whether the current instruction triggers a jump
    ShouldJump,
    /*  Twist/Shout witnesses */
    /// One-hot ra polynomial for the bytecode instance of Shout
    BytecodeRa,
    /// One-hot ra/wa polynomial for the RAM instance of Twist
    /// Note that for RAM, ra and wa are the same polynomial because
    /// there is at most one load or store per cycle.
    /// d = 1 right now hence we only ever use RamRa(0) for now.
    RamRa(usize),
    /// Inc polynomial for the registers instance of Twist
    RdInc,
    /// Inc polynomial for the RAM instance of Twist
    RamInc,
    /// One-hot ra polynomial for the instruction lookups instance of Shout.
    /// There are four (d=4) of these polynomials, `InstructionRa(0) .. InstructionRa(3)`
    InstructionRa(usize),
}

pub const ALL_COMMITTED_POLYNOMIALS: [CommittedPolynomials; 19] = [
    CommittedPolynomials::LeftInstructionInput,
    CommittedPolynomials::RightInstructionInput,
    CommittedPolynomials::Product,
    CommittedPolynomials::WriteLookupOutputToRD,
    CommittedPolynomials::WritePCtoRD,
    CommittedPolynomials::ShouldBranch,
    CommittedPolynomials::ShouldJump,
    CommittedPolynomials::BytecodeRa,
    CommittedPolynomials::RamRa(0),
    CommittedPolynomials::RdInc,
    CommittedPolynomials::RamInc,
    CommittedPolynomials::InstructionRa(0),
    CommittedPolynomials::InstructionRa(1),
    CommittedPolynomials::InstructionRa(2),
    CommittedPolynomials::InstructionRa(3),
    CommittedPolynomials::InstructionRa(4),
    CommittedPolynomials::InstructionRa(5),
    CommittedPolynomials::InstructionRa(6),
    CommittedPolynomials::InstructionRa(7),
];

trait Witness {
    type Type;

    fn generate_witness(&self, cycle: &RV32IMCycle, next_cycle: &RV32IMCycle) -> Self::Type;
}

impl Witness for LeftInstructionInput {
    type Type = u64;

    fn generate_witness(&self, cycle: &RV32IMCycle, next_cycle: &RV32IMCycle) -> Self::Type {
        LookupQuery::<32>::to_instruction_inputs(cycle).0
    }
}

impl Witness for ShouldJump {
    type Type = u8;

    fn generate_witness(&self, cycle: &RV32IMCycle, next_cycle: &RV32IMCycle) -> Self::Type {
        let is_jump = cycle.instruction().circuit_flags()[CircuitFlags::Jump];
        let is_next_noop =
            next_cycle.instruction().circuit_flags()[CircuitFlags::IsNoop];
        is_jump as u8 * (1 - is_next_noop as u8)
    }
}

impl<'a, F: JoltField, PCS: CommitmentScheme<Field = F>> Witness for BytecodeRa<'a, F, PCS> {
    type Type = usize;

    fn generate_witness(&self, cycle: &RV32IMCycle, next_cycle: &RV32IMCycle) -> Self::Type {
        self.preprocessing.shared.bytecode.get_pc(cycle)
    } // TODO: K = preprocessing.shared.bytecode.code_size,
}

impl Witness for RamRa {
    type Type = usize;

    fn generate_witness(&self, cycle: &RV32IMCycle, next_cycle: &RV32IMCycle) -> Self::Type {
        let lookup_index = LookupQuery::<32>::to_lookup_index(cycle);
        let k = (lookup_index
            >> (instruction_lookups::LOG_K_CHUNK
                * (instruction_lookups::D - 1 - self.i)))
            % instruction_lookups::K_CHUNK as u64;
        k as usize
    }
}

pub struct LeftInstructionInput; // (pub u64);
pub struct RightInstructionInput; // (pub i64);
pub struct Product; // (pub u64);
pub struct WriteLookupOutputToRD; // (pub u8);
pub struct WritePCtoRD; // (pub u8);
pub struct ShouldBranch; // (pub u8);
pub struct ShouldJump; // (pub u8);
pub struct BytecodeRa<'a, F: JoltField, PCS: CommitmentScheme<Field = F>> {
    preprocessing: &'a JoltProverPreprocessing<F, PCS>,
}
pub struct RamRa {
    i: usize,
}
pub struct RdInc; // (pub i64);
pub struct RamInc; // (pub i64);
pub struct InstructionRa; // (pub usize);

impl CommittedPolynomials {
    pub fn len() -> usize {
        ALL_COMMITTED_POLYNOMIALS.len()
    }

    pub fn from_index(index: usize) -> Self {
        ALL_COMMITTED_POLYNOMIALS[index]
    }

    pub fn to_index(&self) -> usize {
        ALL_COMMITTED_POLYNOMIALS
            .iter()
            .find_position(|poly| *poly == self)
            .unwrap()
            .0
    }

    pub fn generate_witness<F, PCS>(
        &self,
        preprocessing: &JoltProverPreprocessing<F, PCS>,
        trace: &[RV32IMCycle],
    ) -> MultilinearPolynomial<F>
    where
        F: JoltField,
        PCS: CommitmentScheme<Field = F>,
    {
        match self {
            CommittedPolynomials::LeftInstructionInput => {
                let coeffs: Vec<u64> = trace
                    .par_iter()
                    .map(|cycle| LookupQuery::<32>::to_instruction_inputs(cycle).0)
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::RightInstructionInput => {
                let coeffs: Vec<i64> = trace
                    .par_iter()
                    .map(|cycle| LookupQuery::<32>::to_instruction_inputs(cycle).1)
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::Product => {
                let coeffs: Vec<u64> = trace
                    .par_iter()
                    .map(|cycle| {
                        let (left_input, right_input) =
                            LookupQuery::<32>::to_instruction_inputs(cycle);
                        left_input * right_input as u64
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::WriteLookupOutputToRD => {
                let coeffs: Vec<u8> = trace
                    .par_iter()
                    .map(|cycle| {
                        let flag = cycle.instruction().circuit_flags()
                            [CircuitFlags::WriteLookupOutputToRD as usize];
                        (cycle.rd_write().0 as u8) * (flag as u8)
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::WritePCtoRD => {
                let coeffs: Vec<u8> = trace
                    .par_iter()
                    .map(|cycle| {
                        let flag = cycle.instruction().circuit_flags()[CircuitFlags::Jump as usize];
                        (cycle.rd_write().0 as u8) * (flag as u8)
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::ShouldBranch => {
                let coeffs: Vec<u8> = trace
                    .par_iter()
                    .map(|cycle| {
                        let is_branch =
                            cycle.instruction().circuit_flags()[CircuitFlags::Branch as usize];
                        (LookupQuery::<32>::to_lookup_output(cycle) as u8) * is_branch as u8
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::ShouldJump => {
                let coeffs: Vec<u8> = trace
                    .par_iter()
                    .zip(
                        trace
                            .par_iter()
                            .skip(1)
                            .chain(rayon::iter::once(&RV32IMCycle::NoOp)),
                    )
                    .map(|(cycle, next_cycle)| {
                        let is_jump = cycle.instruction().circuit_flags()[CircuitFlags::Jump];
                        let is_next_noop =
                            next_cycle.instruction().circuit_flags()[CircuitFlags::IsNoop];
                        is_jump as u8 * (1 - is_next_noop as u8)
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::BytecodeRa => {
                let addresses: Vec<usize> = trace
                    .par_iter()
                    .map(|cycle| preprocessing.shared.bytecode.get_pc(cycle))
                    .collect();
                MultilinearPolynomial::OneHot(OneHotPolynomial::from_indices(
                    addresses,
                    preprocessing.shared.bytecode.code_size,
                ))
            }
            // TODO(markosg04) logic here needs to be adjusted for when d > 1 is implemented
            CommittedPolynomials::RamRa(i) => {
                if *i > 0 {
                    panic!("RAM is implemented for only d=1 currently.");
                }
                let addresses: Vec<usize> = trace
                    .par_iter()
                    .map(|cycle| {
                        remap_address(
                            cycle.ram_access().address() as u64,
                            &preprocessing.shared.memory_layout,
                        ) as usize
                    })
                    .collect();
                let K = addresses.par_iter().max().unwrap().next_power_of_two();
                MultilinearPolynomial::OneHot(OneHotPolynomial::from_indices(addresses, K))
            }
            CommittedPolynomials::RdInc => {
                let coeffs: Vec<i64> = trace
                    .par_iter()
                    .map(|cycle| {
                        let (_, pre_value, post_value) = cycle.rd_write();
                        post_value as i64 - pre_value as i64
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::RamInc => {
                let coeffs: Vec<i64> = trace
                    .par_iter()
                    .map(|cycle| {
                        let ram_op = cycle.ram_access();
                        match ram_op {
                            tracer::instruction::RAMAccess::Write(write) => {
                                write.post_value as i64 - write.pre_value as i64
                            }
                            _ => 0,
                        }
                    })
                    .collect();
                coeffs.into()
            }
            CommittedPolynomials::InstructionRa(i) => {
                if *i > instruction_lookups::D {
                    panic!("Unexpected i: {i}");
                }
                let addresses: Vec<usize> = trace
                    .par_iter()
                    .map(|cycle| {
                        let lookup_index = LookupQuery::<32>::to_lookup_index(cycle);
                        let k = (lookup_index
                            >> (instruction_lookups::LOG_K_CHUNK
                                * (instruction_lookups::D - 1 - i)))
                            % instruction_lookups::K_CHUNK as u64;
                        k as usize
                    })
                    .collect();
                MultilinearPolynomial::OneHot(OneHotPolynomial::from_indices(
                    addresses,
                    instruction_lookups::K_CHUNK,
                ))
            }
        }
    }

    pub fn generate_streaming_witness<'a, F, PCS>(
        &self,
        preprocessing: &'a JoltProverPreprocessing<F, PCS>,
        cycle: &RV32IMCycle,
        next_cycle: &RV32IMCycle,
    ) -> StreamingWitness<F>
    where
        F: JoltField,
        PCS: CommitmentScheme<Field = F>,
    {
        match self {
            CommittedPolynomials::LeftInstructionInput => {
                let v = LookupQuery::<32>::to_instruction_inputs(cycle).0;
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::U64Scalars(witness)
            }
            CommittedPolynomials::RightInstructionInput => {
                let v = LookupQuery::<32>::to_instruction_inputs(cycle).1;
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::I64Scalars(witness)
            }
            CommittedPolynomials::Product => {
                let v = {
                    let (left_input, right_input) =
                        LookupQuery::<32>::to_instruction_inputs(cycle);
                    left_input * right_input as u64
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::U64Scalars(witness)
            }
            CommittedPolynomials::WriteLookupOutputToRD => {
                let v = {
                    let flag = cycle.instruction().circuit_flags()
                        [CircuitFlags::WriteLookupOutputToRD as usize];
                    (cycle.rd_write().0 as u8) * (flag as u8)
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::U8Scalars(witness)
            }
            CommittedPolynomials::WritePCtoRD => {
                let v = {
                    let flag = cycle.instruction().circuit_flags()[CircuitFlags::Jump as usize];
                    (cycle.rd_write().0 as u8) * (flag as u8)
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::U8Scalars(witness)
            }
            CommittedPolynomials::ShouldBranch => {
                let v = {
                    let is_branch =
                        cycle.instruction().circuit_flags()[CircuitFlags::Branch as usize];
                    (LookupQuery::<32>::to_lookup_output(cycle) as u8) * is_branch as u8
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::U8Scalars(witness)
            }
            CommittedPolynomials::ShouldJump => {
                let v = {
                    let is_jump = cycle.instruction().circuit_flags()[CircuitFlags::Jump];
                    let is_next_noop =
                        next_cycle.instruction().circuit_flags()[CircuitFlags::IsNoop];
                    is_jump as u8 * (1 - is_next_noop as u8)
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::U8Scalars(witness)
            }
            CommittedPolynomials::BytecodeRa => {
                let v = {
                    preprocessing.shared.bytecode.get_pc(cycle)
                };
                let witness = StreamingOneHotWitness::new(v);
                StreamingWitness::OneHot(witness)
            }
            CommittedPolynomials::RamRa(_) => {
                todo!("This requires doing a full iteration over the trace")
            }
            CommittedPolynomials::RdInc => {
                let v = {
                    let (_, pre_value, post_value) = cycle.rd_write();
                    post_value as i64 - pre_value as i64
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::I64Scalars(witness)
            }
            CommittedPolynomials::RamInc => {
                let v = {
                    let ram_op = cycle.ram_access();
                    match ram_op {
                        tracer::instruction::RAMAccess::Write(write) => {
                            write.post_value as i64 - write.pre_value as i64
                        }
                        _ => 0,
                    }
                };
                let witness = StreamingCompactWitness::new(v);
                StreamingWitness::I64Scalars(witness)
            }
            CommittedPolynomials::InstructionRa(i) => {
                // if *i > instruction_lookups::D {
                //     panic!("Unexpected i: {i}");
                // }
                let v = {
                    let lookup_index = LookupQuery::<32>::to_lookup_index(cycle);
                    let k = (lookup_index
                        >> (instruction_lookups::LOG_K_CHUNK
                            * (instruction_lookups::D - 1 - i)))
                        % instruction_lookups::K_CHUNK as u64;
                    k as usize
                };

                let witness = StreamingOneHotWitness::new(v);
                StreamingWitness::OneHot(witness)
            }
        }
    }
}
