use derive_enum_from_into::EnumFrom;
use iterator_endiate::EndiateIteratorExt;
use tokenizer_lib::{sized_tokens::TokenReaderWithTokenEnds, Token};
use visitable_derive::Visitable;

use super::{ASTNode, Span, TSXToken, TokenReader};
use crate::{
	declarations::{export::Exportable, ExportDeclaration},
	expect_semi_colon,
	marker::MARKER,
	Declaration, Decorated, Marker, ParseOptions, ParseResult, Statement, TSXKeyword, VisitOptions,
	Visitable,
};

#[derive(Debug, Clone, PartialEq, Visitable, get_field_by_type::GetFieldByType, EnumFrom)]
#[get_field_by_type_target(Span)]
#[cfg_attr(feature = "self-rust-tokenize", derive(self_rust_tokenize::SelfRustTokenize))]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[visit_self(under statement)]
pub enum StatementOrDeclaration {
	Statement(Statement),
	Declaration(Declaration),
	/// TODO under cfg
	#[cfg_attr(feature = "self-rust-tokenize", self_tokenize_field(0))]
	Marker(#[visit_skip_field] Marker<Statement>, Span),
}

impl StatementOrDeclaration {
	pub(crate) fn requires_semi_colon(&self) -> bool {
		match self {
			StatementOrDeclaration::Statement(stmt) => stmt.requires_semi_colon(),
			StatementOrDeclaration::Declaration(dec) => matches!(
				dec,
				Declaration::Variable(..)
					| Declaration::Export(Decorated {
						on: ExportDeclaration::Default { .. }
							| ExportDeclaration::Variable {
								exported: Exportable::ImportAll { .. }
									| Exportable::ImportParts { .. } | Exportable::Parts { .. },
								..
							},
						..
					}) | Declaration::Import(..)
			),
			Self::Marker(..) => false,
		}
	}
}

impl ASTNode for StatementOrDeclaration {
	fn get_position(&self) -> &Span {
		match self {
			StatementOrDeclaration::Statement(item) => item.get_position(),
			StatementOrDeclaration::Declaration(item) => item.get_position(),
			StatementOrDeclaration::Marker(_, pos) => pos,
		}
	}

	fn from_reader(
		reader: &mut impl TokenReader<TSXToken, crate::TokenStart>,
		state: &mut crate::ParsingState,
		options: &ParseOptions,
	) -> ParseResult<Self> {
		if options.interpolation_points
			&& matches!(reader.peek(), Some(Token(TSXToken::Identifier(i), _)) if i == MARKER)
		{
			let Token(_, position) = reader.next().unwrap();
			let marker_id = state.new_partial_point_marker(position);
			return Ok(Self::Marker(marker_id, position.with_length(0)));
		}

		if Declaration::is_declaration_start(reader, options) {
			let dec = Declaration::from_reader(reader, state, options)?;
			// TODO nested blocks? Interfaces...?
			Ok(StatementOrDeclaration::Declaration(dec))
		} else {
			if let Some(Token(TSXToken::Keyword(TSXKeyword::Enum | TSXKeyword::Type), _)) =
				reader.peek()
			{
				if reader.peek_n(1).map_or(false, |t| !t.0.is_symbol()) {
					return Ok(StatementOrDeclaration::Declaration(Declaration::from_reader(
						reader, state, options,
					)?));
				}
			}

			let stmt = Statement::from_reader(reader, state, options)?;
			Ok(StatementOrDeclaration::Statement(stmt))
		}
	}

	fn to_string_from_buffer<T: source_map::ToString>(
		&self,
		buf: &mut T,
		options: &crate::ToStringOptions,
		local: crate::LocalToStringInformation,
	) {
		match self {
			StatementOrDeclaration::Statement(item) => {
				item.to_string_from_buffer(buf, options, local);
			}
			StatementOrDeclaration::Declaration(item) => {
				item.to_string_from_buffer(buf, options, local);
			}
			StatementOrDeclaration::Marker(_, _) => {
				panic!()
			}
		}
	}
}

/// A "block" of braced statements and declarations
#[derive(Debug, Clone, get_field_by_type::GetFieldByType)]
#[get_field_by_type_target(Span)]
#[cfg_attr(feature = "self-rust-tokenize", derive(self_rust_tokenize::SelfRustTokenize))]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
pub struct Block(pub Vec<StatementOrDeclaration>, pub Span);

impl Eq for Block {}

impl PartialEq for Block {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

pub struct BlockLike<'a> {
	pub items: &'a Vec<StatementOrDeclaration>,
}

pub struct BlockLikeMut<'a> {
	pub items: &'a mut Vec<StatementOrDeclaration>,
}

impl<'a> From<&'a Block> for BlockLike<'a> {
	fn from(block: &'a Block) -> Self {
		BlockLike { items: &block.0 }
	}
}

impl<'a> From<&'a mut Block> for BlockLikeMut<'a> {
	fn from(block: &'a mut Block) -> Self {
		BlockLikeMut { items: &mut block.0 }
	}
}

impl ASTNode for Block {
	fn from_reader(
		reader: &mut impl TokenReader<TSXToken, crate::TokenStart>,
		state: &mut crate::ParsingState,
		options: &ParseOptions,
	) -> ParseResult<Self> {
		let start = reader.expect_next(TSXToken::OpenBrace)?;
		let items = parse_statements_and_declarations(reader, state, options)?;
		let end_span = reader.expect_next_get_end(TSXToken::CloseBrace)?;
		Ok(Self(items, start.union(end_span)))
	}

	fn to_string_from_buffer<T: source_map::ToString>(
		&self,
		buf: &mut T,
		options: &crate::ToStringOptions,
		local: crate::LocalToStringInformation,
	) {
		buf.push('{');
		if local.depth > 0 && options.pretty {
			buf.push_new_line();
		}
		statements_and_declarations_to_string(&self.0, buf, options, local);
		if options.pretty && !self.0.is_empty() {
			buf.push_new_line();
		}
		if local.depth > 1 {
			options.add_indent(local.depth - 1, buf);
		}
		buf.push('}');
	}

	fn get_position(&self) -> &Span {
		&self.1
	}
}

impl Block {
	pub fn items(&self) -> core::slice::Iter<'_, StatementOrDeclaration> {
		self.0.iter()
	}

	pub fn items_mut(&mut self) -> core::slice::IterMut<'_, StatementOrDeclaration> {
		self.0.iter_mut()
	}
}

impl Visitable for Block {
	fn visit<TData>(
		&self,
		visitors: &mut (impl crate::VisitorReceiver<TData> + ?Sized),
		data: &mut TData,
		options: &VisitOptions,
		chain: &mut temporary_annex::Annex<crate::visiting::Chain>,
	) {
		if options.visit_nested_blocks || chain.is_empty() {
			{
				visitors.visit_block(&crate::block::BlockLike { items: &self.0 }, data, chain);
			}
			let items = self.items();
			if options.reverse_statements {
				items.rev().for_each(|item| item.visit(visitors, data, options, chain));
			} else {
				items.for_each(|item| item.visit(visitors, data, options, chain));
			}
		}
	}

	fn visit_mut<TData>(
		&mut self,
		visitors: &mut (impl crate::VisitorMutReceiver<TData> + ?Sized),
		data: &mut TData,
		options: &VisitOptions,
		chain: &mut temporary_annex::Annex<crate::visiting::Chain>,
	) {
		if options.visit_nested_blocks || chain.is_empty() {
			{
				visitors.visit_block_mut(
					&mut crate::block::BlockLikeMut { items: &mut self.0 },
					data,
					chain,
				);
			}
			let items = self.items_mut();
			if options.reverse_statements {
				items.for_each(|statement| statement.visit_mut(visitors, data, options, chain));
			} else {
				items
					.rev()
					.for_each(|statement| statement.visit_mut(visitors, data, options, chain));
			}
		}
	}
}

/// For ifs and other statements
#[derive(Debug, Clone, PartialEq, Eq, EnumFrom)]
#[cfg_attr(feature = "self-rust-tokenize", derive(self_rust_tokenize::SelfRustTokenize))]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
pub enum BlockOrSingleStatement {
	Braced(Block),
	SingleStatement(Box<Statement>),
}

impl Visitable for BlockOrSingleStatement {
	fn visit<TData>(
		&self,
		visitors: &mut (impl crate::visiting::VisitorReceiver<TData> + ?Sized),
		data: &mut TData,
		options: &VisitOptions,
		chain: &mut temporary_annex::Annex<crate::visiting::Chain>,
	) {
		match self {
			BlockOrSingleStatement::Braced(b) => {
				b.visit(visitors, data, options, chain);
			}
			BlockOrSingleStatement::SingleStatement(s) => {
				s.visit(visitors, data, options, chain);
				visitors.visit_statement(
					crate::visiting::BlockItem::SingleStatement(s),
					data,
					chain,
				);
			}
		}
	}

	fn visit_mut<TData>(
		&mut self,
		visitors: &mut (impl crate::visiting::VisitorMutReceiver<TData> + ?Sized),
		data: &mut TData,
		options: &VisitOptions,
		chain: &mut temporary_annex::Annex<crate::visiting::Chain>,
	) {
		match self {
			BlockOrSingleStatement::Braced(ref mut b) => {
				b.visit_mut(visitors, data, options, chain);
			}
			BlockOrSingleStatement::SingleStatement(ref mut s) => {
				s.visit_mut(visitors, data, options, chain);
				visitors.visit_statement_mut(
					crate::visiting::BlockItemMut::SingleStatement(s),
					data,
					chain,
				);
			}
		}
	}
}

impl From<Statement> for BlockOrSingleStatement {
	fn from(stmt: Statement) -> Self {
		Self::SingleStatement(Box::new(stmt))
	}
}

impl ASTNode for BlockOrSingleStatement {
	fn get_position(&self) -> &Span {
		match self {
			BlockOrSingleStatement::Braced(blk) => blk.get_position(),
			BlockOrSingleStatement::SingleStatement(stmt) => stmt.get_position(),
		}
	}

	fn from_reader(
		reader: &mut impl TokenReader<TSXToken, crate::TokenStart>,
		state: &mut crate::ParsingState,
		options: &ParseOptions,
	) -> ParseResult<Self> {
		let stmt = Statement::from_reader(reader, state, options)?;
		Ok(match stmt {
			Statement::Block(blk) => Self::Braced(blk),
			stmt => {
				if stmt.requires_semi_colon() {
					expect_semi_colon(reader, &state.line_starts, stmt.get_position().end)?;
				}
				Box::new(stmt).into()
			}
		})
	}

	fn to_string_from_buffer<T: source_map::ToString>(
		&self,
		buf: &mut T,
		options: &crate::ToStringOptions,
		local: crate::LocalToStringInformation,
	) {
		if buf.should_halt() {
			return;
		}
		match self {
			BlockOrSingleStatement::Braced(block) => {
				block.to_string_from_buffer(buf, options, local);
			}
			BlockOrSingleStatement::SingleStatement(stmt) => {
				if options.pretty && !options.single_statement_on_new_line {
					buf.push_new_line();
					options.push_gap_optionally(buf);
					stmt.to_string_from_buffer(buf, options, local.next_level());
				} else {
					stmt.to_string_from_buffer(buf, options, local);
					if stmt.requires_semi_colon() {
						buf.push(';');
					}
				}
			}
		}
	}
}

/// Parse statements, regardless of bracing or not
pub(crate) fn parse_statements_and_declarations(
	reader: &mut impl TokenReader<TSXToken, crate::TokenStart>,
	state: &mut crate::ParsingState,
	options: &ParseOptions,
) -> ParseResult<Vec<StatementOrDeclaration>> {
	let mut items = Vec::new();
	while let Some(Token(token_type, _)) = reader.peek() {
		if let TSXToken::EOS | TSXToken::CloseBrace = token_type {
			break;
		}

		let value = StatementOrDeclaration::from_reader(reader, state, options)?;
		if value.requires_semi_colon() {
			expect_semi_colon(reader, &state.line_starts, value.get_position().end)?;
		} else {
			// Skip over semi colons regardless
			// reader.conditional_next(|t| matches!(t, TSXToken::SemiColon));
		}
		items.push(value);
	}
	Ok(items)
}

pub fn statements_and_declarations_to_string<T: source_map::ToString>(
	items: &[StatementOrDeclaration],
	buf: &mut T,
	options: &crate::ToStringOptions,
	local: crate::LocalToStringInformation,
) {
	for (at_end, item) in items.iter().endiate() {
		if !options.pretty {
			if let StatementOrDeclaration::Statement(Statement::Expression(
				crate::expressions::MultipleExpression::Single(crate::Expression::Null(..)),
			)) = item
			{
				continue;
			}
		}

		options.add_indent(local.depth, buf);
		item.to_string_from_buffer(buf, options, local);
		if (!at_end || options.trailing_semicolon) && item.requires_semi_colon() {
			buf.push(';');
		}
		// TODO only append new line if something added
		if !at_end && options.pretty {
			buf.push_new_line();
		}
	}
}
