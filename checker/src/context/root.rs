use super::{
	environment::DynamicBoundaryKind, ClosedOverReferencesInScope, Context, ContextId, ContextType,
};
use crate::{
	features::{
		modules::{Exported, SynthesisedModule},
		variables::VariableOrImport,
	},
	types::TypeId,
	CheckingData, GeneralContext,
};
use source_map::SourceId;
use std::{collections::HashMap, iter::FromIterator};

pub type RootContext = Context<Root>;

#[derive(Debug)]
pub struct Root;

impl ContextType for Root {
	fn as_general_context(et: &Context<Self>) -> GeneralContext<'_> {
		GeneralContext::Root(et)
	}

	fn get_parent(&self) -> Option<&GeneralContext<'_>> {
		None
	}

	fn is_dynamic_boundary(&self) -> Option<DynamicBoundaryKind> {
		None
	}

	fn is_conditional(&self) -> bool {
		false
	}

	fn get_closed_over_references(&mut self) -> Option<&mut ClosedOverReferencesInScope> {
		None
	}

	fn get_exports(&mut self) -> Option<&mut Exported> {
		None
	}
}

const _CONTEXT_FILE_HEADER: &[u8] = b"EZNO\0CONTEXT\0FILE";

impl RootContext {
	/// Merges two [`RootEnvironments`]. May be used for multiple `.d.ts` files
	pub(crate) fn _union(&mut self, other: Self) {
		// TODO this is bad, some things need to merge, inserting over existing will be bad
		self.variables.extend(other.variables);
		todo!()
		// self.tys.extend(other.tys.into_iter());
	}

	#[must_use]
	pub fn new_with_primitive_references() -> Self {
		// TODO number might not be a reference at some point
		let types = [
			("number".to_owned(), TypeId::NUMBER_TYPE),
			("string".to_owned(), TypeId::STRING_TYPE),
			("boolean".to_owned(), TypeId::BOOLEAN_TYPE),
			("null".to_owned(), TypeId::NULL_TYPE),
			("undefined".to_owned(), TypeId::UNDEFINED_TYPE),
			("void".to_owned(), TypeId::VOID_TYPE),
			("Array".to_owned(), TypeId::ARRAY_TYPE),
			("Function".to_owned(), TypeId::FUNCTION_TYPE),
			("object".to_owned(), TypeId::OBJECT_TYPE),
		];

		let mut facts = crate::Facts::default();

		// Add undefined
		let variables = {
			let variable_or_import = VariableOrImport::Variable {
				mutability: crate::features::variables::VariableMutability::Constant,
				declared_at: source_map::Nullable::NULL,
				context: None,
			};
			let undefined_id = variable_or_import.get_id();
			let variables = [("undefined".to_owned(), variable_or_import)];
			facts.variable_current_value.insert(undefined_id, TypeId::UNDEFINED_TYPE);
			variables
		};

		Self {
			context_type: Root,
			context_id: ContextId::ROOT,
			named_types: HashMap::from_iter(types),
			variables: HashMap::from_iter(variables),
			variable_names: Default::default(),
			deferred_function_constraints: Default::default(),
			bases: Default::default(),
			object_constraints: Default::default(),
			// TODO
			can_reference_this: crate::context::CanReferenceThis::Yeah,
			facts,
			possibly_mutated_objects: Default::default(),
		}
	}

	pub fn new_module_context<'a, T: crate::ReadFromFS, A: crate::ASTImplementation>(
		&self,
		source: SourceId,
		module: A::Module<'static>,
		checking_data: &'a mut CheckingData<T, A>,
	) -> &'a SynthesisedModule<A::OwnedModule> {
		let module_scope = crate::Scope::Module { source, exported: Exported::default() };
		let mut environment = self.new_lexical_environment(module_scope);
		A::synthesise_module(&module, source, &mut environment, checking_data);

		let crate::Scope::Module { exported, .. } = environment.context_type.scope else {
			unreachable!()
		};

		let module = SynthesisedModule {
			content: A::owned_module_from_module(module),
			exported,
			facts: environment.facts,
		};

		// TODO better way to do this?
		checking_data.modules.synthesised_modules.insert(source, module);
		checking_data.modules.synthesised_modules.get(&source).unwrap()
	}

	/// TODO working things out:
	/// - strings could reference a big string
	#[must_use]
	pub fn serialize(self) -> Vec<u8> {
		todo!()
	}

	pub fn deserialize(_source: &[u8], _backing_source: SourceId) -> Result<Self, String> {
		todo!()
		// let mut ctx = Root::new_with_primitive_references();

		// if !source.starts_with(HEADER) {
		// 	return Err("Missing header".to_owned());
		// }

		// let mut bytes = source.into_iter();

		// {
		// 	assert_eq!(bytes.by_ref().take(HEADER.len()).collect::<Vec<_>>(), HEADER);
		// }

		// // Types
		// let count = u16::from_le_bytes([bytes.next().unwrap(), bytes.next().unwrap()]);

		// for _ in 0..count {
		// 	let ty = Type::deserialize(&mut bytes, backing_source);
		// 	ctx.new_type(ty);
		// }
		// crate::utils::notify!("Registered {:?} types", count);

		// ctx.variables = BinarySerializable::deserialize(&mut bytes, backing_source);
		// // TODO terrible
		// VariableId::set_counter_bad((ctx.variables.len() + 1) as u16);
		// ctx.variable_names = BinarySerializable::deserialize(&mut bytes, backing_source);

		// ctx.proofs = BinarySerializable::deserialize(&mut bytes, backing_source);
		// ctx.functions_on_type = BinarySerializable::deserialize(&mut bytes, backing_source);
		// ctx.subtyping_constant_proofs = BinarySerializable::deserialize(&mut bytes, backing_source);
		// ctx.terms_reverse = BinarySerializable::deserialize(&mut bytes, backing_source);
		// ctx.proxies = BinarySerializable::deserialize(&mut bytes, backing_source);
		// ctx.can_reference_this = BinarySerializable::deserialize(&mut bytes, backing_source);

		// Ok(ctx)
	}
}
