use jolt_core::zkvm::instruction::InstructionLookup;
use strum::IntoEnumIterator as _;
use tracer::instruction::RV32IMInstruction;

use crate::{
    constants::JoltParameterSet,
    modules::{AsModule, Module},
    util::{indent, ZkLeanReprField},
    MleAst,
};

/// Wrapper around a JoltInstruction
// TODO: Make this generic over the instruction set
#[derive(Debug, Clone)]
pub struct ZkLeanInstruction<J> {
    instruction: RV32IMInstruction,
    phantom: std::marker::PhantomData<J>,
}

impl<J> From<RV32IMInstruction> for ZkLeanInstruction<J> {
    fn from(value: RV32IMInstruction) -> Self {
        Self {
            instruction: value,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<J: JoltParameterSet> ZkLeanInstruction<J> {
    pub fn name(&self) -> String {
        let name = <&'static str>::from(&self.instruction);
        let word_size = J::WORD_SIZE;

        format!("{name}_{word_size}")
    }

    pub fn iter() -> impl Iterator<Item = Self> {
        //RV32IMInstruction::iter().map(Self::from)
        use tracer::instruction::*;
        [
            ZkLeanInstruction::from(RV32IMInstruction::AND(and::AND::default())),
            ZkLeanInstruction::from(RV32IMInstruction::OR(or::OR::default())),
            ZkLeanInstruction::from(RV32IMInstruction::XOR(xor::XOR::default())),
            ZkLeanInstruction::from(RV32IMInstruction::VirtualSRL(virtual_srl::VirtualSRL::default())),
            ZkLeanInstruction::from(RV32IMInstruction::VirtualSRA(virtual_sra::VirtualSRA::default())),
        ].into_iter()
    }

    pub fn evaluate_mle<F: ZkLeanReprField>(&self, reg_name: char) -> Option<F> {
        let reg = F::register(reg_name, 2 * J::WORD_SIZE);
        self.instruction
            .lookup_table()
            .map(|instr| instr.evaluate_mle(&reg))
    }

    /// Pretty print an instruction as a ZkLean `ComposedLookupTable`.
    pub fn zklean_pretty_print<F: ZkLeanReprField>(
        &self,
        f: &mut impl std::io::Write,
        mut indent_level: usize,
    ) -> std::io::Result<()> {
        let name = self.name();
        let reg_size = 2 * J::WORD_SIZE;
        self.evaluate_mle::<F>('x')
            .map_or(Ok(()), |mle| {
                f.write_fmt(format_args!(
                        "{}def {name} [Field f] : Subtable f {reg_size} :=\n",
                        indent(indent_level),
                ))?;
                indent_level += 1;
                f.write_fmt(format_args!(
                        "{}subtableFromMLE (fun x => {mle})\n",
                        indent(indent_level),
                ))?;

                Ok(())
            })
    }
}

pub struct ZkLeanInstructions<J> {
    instructions: Vec<ZkLeanInstruction<J>>,
}

impl<J: JoltParameterSet> ZkLeanInstructions<J> {
    pub fn extract() -> Self {
        Self {
            instructions: ZkLeanInstruction::<J>::iter().collect(),
        }
    }

    pub fn zklean_pretty_print(
        &self,
        f: &mut impl std::io::Write,
        indent_level: usize,
    ) -> std::io::Result<()> {
        for instruction in &self.instructions {
            instruction.zklean_pretty_print::<MleAst<6000>>(f, indent_level)?;
        }
        Ok(())
    }

    pub fn zklean_imports(&self) -> Vec<String> {
        vec![String::from("ZkLean"), String::from("Jolt.Subtables")]
    }
}

impl<J: JoltParameterSet> AsModule for ZkLeanInstructions<J> {
    fn as_module(&self) -> std::io::Result<Module> {
        let mut contents: Vec<u8> = vec![];
        self.zklean_pretty_print(&mut contents, 0)?;

        Ok(Module {
            name: String::from("Instructions"),
            imports: self.zklean_imports(),
            contents,
        })
    }
}

//#[cfg(test)]
//mod test {
//    use super::*;
//    use crate::util::arb_field_elem;
//
//    use jolt_core::field::JoltField;
//
//    use proptest::{collection::vec, prelude::*};
//    use strum::EnumCount as _;
//
//    type RefField = ark_bn254::Fr;
//    type TestField = crate::mle_ast::MleAst<2048>;
//    type ParamSet = crate::constants::RV32IParameterSet;
//
//    #[derive(Clone)]
//    struct TestableInstruction<J: JoltParameterSet> {
//        reference: RV32I,
//        test: ZkLeanInstruction<J>,
//    }
//
//    impl<J: JoltParameterSet> std::fmt::Debug for TestableInstruction<J> {
//        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//            f.write_fmt(format_args!("{}", self.test.name()))
//        }
//    }
//
//    impl<J: JoltParameterSet> TestableInstruction<J> {
//        fn iter() -> impl Iterator<Item = Self> {
//            RV32I::iter()
//                .zip(ZkLeanInstruction::iter())
//                .map(|(reference, test)| Self { reference, test })
//        }
//
//        fn reference_combine_lookups<R: JoltField>(&self, inputs: &[R]) -> R {
//            assert_eq!(inputs.len(), self.test.num_lookups::<R>());
//
//            self.reference.combine_lookups(inputs, J::C, J::M)
//        }
//
//        fn test_combine_lookups<R: JoltField, T: ZkLeanReprField>(&self, inputs: &[R]) -> R {
//            assert_eq!(inputs.len(), self.test.num_lookups::<R>());
//
//            let ast: T = self.test.combine_lookups('x');
//            ast.evaluate(inputs)
//        }
//    }
//
//    fn arb_instruction<J: JoltParameterSet>() -> impl Strategy<Value = TestableInstruction<J>> {
//        (0..RV32I::COUNT).prop_map(|n| TestableInstruction::iter().nth(n).unwrap())
//    }
//
//    fn arb_instruction_and_input<J: JoltParameterSet + Clone, R: JoltField>(
//    ) -> impl Strategy<Value = (TestableInstruction<J>, Vec<R>)> {
//        arb_instruction().prop_flat_map(|instr| {
//            let input_len = instr.test.num_lookups::<R>();
//            let inputs = vec(arb_field_elem::<R>(), input_len);
//
//            (Just(instr), inputs)
//        })
//    }
//
//    proptest! {
//        #[test]
//        fn combine_lookups(
//            (instr, inputs) in arb_instruction_and_input::<ParamSet, RefField>(),
//        ) {
//            prop_assert_eq!(
//                instr.test_combine_lookups::<_, TestField>(&inputs),
//                instr.reference_combine_lookups(&inputs),
//            );
//        }
//    }
//}
