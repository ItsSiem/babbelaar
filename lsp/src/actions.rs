// Copyright (C) 2024 Tristan Gerritsen <tristan@thewoosh.org>
// All Rights Reserved.

use std::collections::HashMap;

use babbelaar::{BabbelaarCodeAction, BabbelaarCodeActionType, BiExpression, Expression, FileEdit, FileLocation, FileRange, FunctionCallExpression, ParseTree, PostfixExpression, PostfixExpressionKind, PrimaryExpression, SemanticAnalyzer, Statement, StatementKind, StructureInstantiationExpression, TemplateStringExpressionPart};
use tower_lsp::lsp_types::VersionedTextDocumentIdentifier;

use crate::BabbelaarLspError;

#[derive(Debug)]
pub struct CodeActionItem {
    pub action: BabbelaarCodeAction,
    pub document: VersionedTextDocumentIdentifier,
}

#[derive(Debug)]
pub struct CodeActionRepository {
    id_tracker: usize,

    /// TODO: find some way to invalidate & free the code actions when they're not used anymore,
    ///       to avoid memory exhaustion. E.g. when the document has x amount of edits, we discard
    ///       the old code actions.
    values: HashMap<usize, CodeActionItem>,
}

impl CodeActionRepository {
    pub fn new() -> Self {
        Self {
            id_tracker: 1,
            values: HashMap::new(),
        }
    }

    #[must_use]
    pub fn add(&mut self, action: BabbelaarCodeAction, document: VersionedTextDocumentIdentifier) -> usize {
        let id = self.id_tracker;
        self.id_tracker += 1;
        self.values.insert(id, CodeActionItem {
            action,
            document,
        });
        id
    }

    #[must_use]
    pub fn get(&self, id: usize) -> Option<&CodeActionItem> {
        self.values.get(&id)
    }
}

pub struct CodeActionsAnalysisContext<'ctx> {
    pub semantics: &'ctx SemanticAnalyzer<'ctx>,
    pub items: Vec<BabbelaarCodeAction>,
    pub cursor_range: FileRange,
    pub contents: &'ctx str,
}

impl<'ctx> CodeActionsAnalysisContext<'ctx> {
    pub fn create_range_and_calculate_byte_column(&self, start: FileLocation, end: FileLocation) -> Result<FileRange, BabbelaarLspError> {
        let line = self.contents.lines().nth(start.line())
            .ok_or_else(|| BabbelaarLspError::InvalidDataSent {
                explanation: format!("er is geen lijn op {}, we hebben er {}. De laatste is \"{:?}\"", start.line(), self.contents.lines().count(), self.contents.lines().last())
            })?;

        let start = map_column_from_char_to_byte_offset(line, start);
        let end = map_column_from_char_to_byte_offset(line, end);

        Ok(FileRange::new(start, end))
    }

    pub fn indentation_at(&self, start: FileLocation) -> Option<&'ctx str> {
        let line = self.contents.lines().nth(start.line())?;

        let line = &line[..start.column()];
        Some(&line[..line.len() - line.trim_start().len()])
    }
}

fn map_column_from_char_to_byte_offset(line: &str, location: FileLocation) -> FileLocation {
    let column = line.char_indices()
        .nth(location.column())
        .unwrap_or((line.len(), ' '))
        .0;

    FileLocation::new(location.offset(), location.line(), column)
}

pub trait CodeActionsAnalyzable {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>);
}

impl<'source_code> CodeActionsAnalyzable for BiExpression<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        if self.lhs.range().contains(ctx.cursor_range.start()) {
            self.lhs.analyze(ctx);
        }

        if self.rhs.range().contains(ctx.cursor_range.start()) {
            self.rhs.analyze(ctx);
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for Expression<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        match self {
            Self::Primary(expr) => expr.analyze(ctx),
            Self::BiExpression(expr) => expr.analyze(ctx),
            Self::Postfix(expr) => expr.analyze(ctx),
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for FunctionCallExpression<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        let range = FileRange::new(self.token_left_paren.start(), self.token_right_paren.end());
        if !range.contains(ctx.cursor_range.start()) {

            return;
        }

        for arg in &self.arguments {
            if arg.range().contains(ctx.cursor_range.start()) {
                arg.analyze(ctx);
            }
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for ParseTree<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        for stmt in self.all() {
            stmt.analyze(ctx);
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for PostfixExpression<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        if self.lhs.range().contains(ctx.cursor_range.start()) {
            self.lhs.analyze(ctx);
        }

        match &self.kind {
            PostfixExpressionKind::Call(expr) => {
                expr.analyze(ctx);
            }

            PostfixExpressionKind::Member(..) => (),

            PostfixExpressionKind::MethodCall(method) => {
                method.call.analyze(ctx);
            }
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for PrimaryExpression<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        match self {
            Self::Boolean(..) => (),
            Self::IntegerLiteral(..) => (),

            Self::Parenthesized(expr) => {
                expr.analyze(ctx);
            }

            Self::Reference(..) => (),
            Self::StringLiteral(..) => (),

            Self::StructureInstantiation(structure) => {
                structure.analyze(ctx);
            }

            Self::TemplateString { parts } => {
                for part in parts {
                    if let TemplateStringExpressionPart::Expression(expr) = part {
                        expr.analyze(ctx);
                    }
                }
            }
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for Statement<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        match &self.kind {
            StatementKind::Expression(expr) => {
                expr.analyze(ctx);
                // TODO check unused value
            }

            StatementKind::Assignment(stmt) => {
                stmt.expression.analyze(ctx);
            }

            StatementKind::For(stmt) => {
                for stmt in &stmt.body {
                    stmt.analyze(ctx);
                }
            }

            StatementKind::Function(func) => {
                for stmt in func.body.as_ref().map(Vec::as_slice).unwrap_or_default() {
                    stmt.analyze(ctx);
                }
            }

            StatementKind::If(stmt) => {
                stmt.condition.analyze(ctx);
                for stmt in &stmt.body {
                    stmt.analyze(ctx);
                }
            }

            StatementKind::Return(stmt) => {
                if let Some(expr) = &stmt.expression {
                    expr.analyze(ctx);
                }
            }

            StatementKind::Structure(stmt) => {
                _ = stmt;
                // TODO
            }

            StatementKind::Variable(stmt) => {
                stmt.expression.analyze(ctx);
            }
        }
    }
}

impl<'source_code> CodeActionsAnalyzable for StructureInstantiationExpression<'source_code> {
    fn analyze(&self, ctx: &mut CodeActionsAnalysisContext<'_>) {
        if !self.range.contains(ctx.cursor_range.start()) {
            return;
        }

        for field in &self.fields {
            field.value.analyze(ctx);
        }

        let indent = ctx.indentation_at(self.name.range().start()).unwrap_or("invalid").to_string() + "    ";

        ctx.semantics.scopes_surrounding(self.name.range().start(), |scope| {
            if let Some(structure) = scope.structures.get(self.name.value()) {
                let mut str = String::new();

                for field in structure.fields.iter() {
                    if self.fields.iter().find(|x| x.name.value() == field.name.value()).is_some() {
                        continue;
                    }

                    str += "\n";
                    str += &indent;

                    str += field.name.value();
                    str += ": ";
                    str += field.ty.default_value_hint();
                    str += ",";
                }

                str += "\n";

                if !str.trim().is_empty() {
                    let edit = FileEdit::new(FileRange::new(self.left_curly_bracket.end(), self.right_curly_bracket.start()), str);

                    ctx.items.push(BabbelaarCodeAction::new(
                        BabbelaarCodeActionType::FillStructureFields { structure: structure.name.to_string() },
                        vec![edit],
                    ))
                }
            }
        })
    }
}