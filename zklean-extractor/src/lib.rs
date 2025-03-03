extern crate proc_macro;
use proc_macro::TokenStream;
use syn::{parse::{Parse, ParseStream}, parse_macro_input, punctuated::Punctuated, Token};
use proc_macro2::{Ident, Literal};
use quote::{format_ident, quote};

/// Parser for a proc-macro enum declaration. Parses an enum declaration, consisting of an
/// identifier (for the resulting enum type), followed by a comma, and then a comma-separated
/// sequence of types, each of which will become a variant, named after the type, and containing an
/// item of that type.
#[derive(Debug, Clone)]
struct EnumParser {
    id: Ident,
    entries: Punctuated<VariantParser, Token![,]>,
}

impl Parse for EnumParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let id = Ident::parse(input)?;
        let _ = <Token![,]>::parse(input)?;
        let entries: Punctuated<VariantParser, Token![,]> = Punctuated::parse_terminated(input)?;
        Ok(Self { id, entries })
    }
}

/// Parser for a variant within a proc-macro enum declaration. Contains a type, possibly with a
/// module path and a collection of const generics in angle brackets.
#[derive(Debug, Clone)]
struct VariantParser {
    path: Punctuated<Ident, Token![::]>,
    id: Ident,
    const_generics: Punctuated<Literal, Token![,]>,
}

impl VariantParser {
    fn to_ident(&self) -> Ident {
        let mut res = self.id.clone();
        for c in &self.const_generics {
            res = format_ident!("{res}_{}", c.to_string());
        }
        res
    }
}

impl Parse for VariantParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut path = Punctuated::<Ident, Token![::]>::new();
        let mut id = Ident::parse(input)?;
        loop {
            if input.peek(Token![::]) {
                path.push_value(id.clone());
                path.push_punct(<Token![::]>::parse(input)?);
                id = Ident::parse(input)?;
            } else {
                break;
            }
        }
        let const_generics = if input.peek(Token![<]) {
            let _ = <Token![<]>::parse(input)?;
            let gs = Punctuated::<Literal, Token![,]>::parse_separated_nonempty(input)?;
            let _ = <Token![>]>::parse(input)?;
            gs
        } else {
            Punctuated::<Literal, Token![,]>::new()
        };
        Ok(Self { path, id, const_generics })
    }
}

/// Declare an enum of subtable types.
#[proc_macro]
pub fn declare_subtables_enum(input: TokenStream) -> TokenStream {
    let EnumParser { id: enum_id, entries } = parse_macro_input!(input as EnumParser);
    let mut variants = vec![];
    let mut name_cases = vec![];
    let mut eval_cases = vec![];
    let mut enum_cases = vec![];
    let mut conv_conditions = vec![];
    let mut tests = vec![];

    for entry in entries {
        let path = entry.path.clone();
        let id = entry.id.clone();
        let name = entry.to_ident();
        let name_str = name.to_string();
        let const_generics = entry.const_generics;

        variants.push(quote! {
            #[allow(non_camel_case_types)]
            #name(#path #id<F, #const_generics>)
        });
        name_cases.push(quote! {
            Self::#name(_) => #name_str
        });
        eval_cases.push(quote! {
            Self::#name(s) => s.evaluate_mle(&vars)
        });
        enum_cases.push(quote! {
            Self::#name(#path #id::new())
        });
        conv_conditions.push(quote! {
            if t == #path #id::<F, #const_generics>::new().subtable_id() {
                return Self::#name(#path #id::<F, #const_generics>::new());
            }
        });
        tests.push(quote! {
            /// Test that extracting the subtable as an `crate::mle_ast::MleAst` and evaluating it
            /// results in the same value as simply evaluating it would.
            #[test]
            #[allow(non_snake_case)]
            fn #name(values_u64 in proptest::collection::vec(proptest::num::u64::ANY, 8)) {
                type RefField = jolt_core::field::binius::BiniusField<binius_field::BinaryField128b>;
                type AstField = crate::mle_ast::MleAst<2048>;
                let (actual, expected, mle) = crate::util::test_evaluate_fn(
                    &values_u64,
                    #path #id::<RefField, #const_generics>::new(),
                    #path #id::<AstField, #const_generics>::new(),
                );
                prop_assert_eq!(actual, expected, "\n   mle: {}:", mle);
            }
        });
    }

    quote! {
        pub enum #enum_id<F: crate::util::ZkLeanReprField> {
            #(#variants),*
        }

        impl<F: crate::util::ZkLeanReprField> #enum_id<F> {
            /// Name of this subtable variant, incorporating the type and any const-generics.
            pub fn name(&self) -> &'static str {
                match self {
                    #(#name_cases),*
                }
            }

            /// Call the `evaluate_mle` method on the contained subtable.
            pub fn evaluate_mle(&self, reg_name: char, reg_size: usize) -> F {
                use jolt_core::jolt::subtable::LassoSubtable;
                use crate::util::ZkLeanReprField;
                let vars = F::register(reg_name, reg_size);
                match self {
                    #(#eval_cases),*
                }
            }

            /// Enumerate all variants.
            pub fn enumerate() -> Vec<Self> {
                vec![
                    #(#enum_cases),*
                ]
            }

            /// Construct a new object of the appropriate type given a subtable's
            /// [`std::any::TypeId`].
            pub fn from_subtable_id(t: std::any::TypeId) -> Self {
                use jolt_core::jolt::subtable::LassoSubtable;
                #(#conv_conditions)*
                panic!("Unimplemented conversion from {t:?}")
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use proptest::prelude::*;
            proptest! {
                #(#tests)*
            }
        }
    }.into()
}

/// Declare an enum of instruction types.
#[proc_macro]
pub fn declare_instructions_enum(input: TokenStream) -> TokenStream {
    let EnumParser { id: enum_id, entries } = parse_macro_input!(input as EnumParser);
    let mut variants = vec![];
    let mut name_cases = vec![];
    let mut combine_cases = vec![];
    let mut subtables_cases = vec![];
    let mut enum_cases = vec![];
    //let mut tests = vec![];

    for entry in entries {
        let path = entry.path.clone();
        let id = entry.id.clone();
        let name = entry.to_ident();
        let name_str = name.to_string();
        let const_generics = entry.const_generics;

        variants.push(quote! {
            #[allow(non_camel_case_types)]
            #name(#path #id<WORD_SIZE, #const_generics>)
        });
        name_cases.push(quote! {
            Self::#name(_) => #name_str
        });
        combine_cases.push(quote! {
            Self::#name(i) => i.combine_lookups(&vars, c, m)
        });
        subtables_cases.push(quote! {
            Self::#name(i) => i.subtables(c, m)
        });
        // FIXME: How do we handle the arguments in the instantiation?
        enum_cases.push(quote! {
            Self::#name(#path #id(42, 9001))
        });
        // TODO: tests
        //tests.push(quote! {
        //    #[test]
        //    #[allow(non_snake_case)]
        //    fn #name(values_u64 in proptest::collection::vec(proptest::num::u64::ANY, 8)) {
        //        type RefField = jolt_core::field::binius::BiniusField<binius_field::BinaryField128b>;
        //        type AstField = crate::mle_ast::MleAst<2048>;
        //        let (actual, expected, mle) = crate::util::test_evaluate_fn(
        //            &values_u64,
        //            #path #id::<RefField, #const_generics>::new(),
        //            #path #id::<AstField, #const_generics>::new(),
        //        );
        //        prop_assert_eq!(actual, expected, "\n   mle: {}:", mle);
        //    }
        //});
    }

    quote! {
        pub enum #enum_id<const WORD_SIZE: usize> {
            #(#variants),*
        }

        impl<const WORD_SIZE: usize> #enum_id<WORD_SIZE> {
            /// Name of this instruction variant, incorporating the type and any const generics.
            pub fn name(&self) -> &'static str {
                match self {
                    #(#name_cases),*
                }
            }

            /// Call the `combine_lookups` method on the underlying instruction.
            pub fn combine_lookups<F: crate::util::ZkLeanReprField>(&self, reg_name: char, c: usize, m: usize) -> F {
                use jolt_core::jolt::instruction::JoltInstruction;

                // Count total subtable evaluations required
                let reg_size = self.subtables::<F>(c, m).len();
                let vars = F::register(reg_name, reg_size);

                match self {
                    #(#combine_cases),*
                }
            }

            /// Call the `subtables` method on the underlying instruction.
            pub fn subtables<F: crate::util::ZkLeanReprField>(&self, c: usize, m: usize) -> Vec<(String, usize)> {
                use jolt_core::jolt::{instruction::{JoltInstruction, SubtableIndices}, subtable::LassoSubtable};
                use crate::subtable::NamedSubtable;

                let subtables: Vec<(Box<dyn LassoSubtable<F>>, SubtableIndices)> = match self {
                    #(#subtables_cases),*
                };

                let mut res = vec![];
                for (subtable, indices) in subtables {
                    // FIXME: Referencing `NamedSubtable` here directly isn't great.
                    let subtable = NamedSubtable::<F>::from_subtable_id(subtable.subtable_id());
                    let subtable_name = subtable.name().to_string();
                    for i in indices.iter() {
                        res.push((subtable_name.clone(), i));
                    }
                }
                res
            }

            /// Enumerate all variants.
            pub fn enumerate() -> Vec<Self> {
                vec![
                    #(#enum_cases),*
                ]
            }
        }

        //#[cfg(test)]
        //mod tests {
        //    use super::*;
        //    use proptest::prelude::*;
        //    proptest! {
        //        #(#tests)*
        //    }
        //}
    }.into()
}
