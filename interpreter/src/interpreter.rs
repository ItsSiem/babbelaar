// Copyright (C) 2023 Tristan Gerritsen <tristan@thewoosh.org>
// All Rights Reserved.

use std::{
    collections::HashMap,
    rc::Rc,
};

use crate::*;

pub struct Interpreter<'source_code> {
    functions: HashMap<&'source_code str, Rc<FunctionStatement<'source_code>>>,
    scope: Scope<'source_code>,
}

impl<'source_code> Interpreter<'source_code> {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            scope: Scope::new(),
        }
    }

    pub fn execute(&mut self, statement: &Statement<'source_code>) {
        match statement {
            Statement::Expression(expression) => {
                self.execute_expression(expression);
            }

            Statement::For(statement) => {
                self.execute_for_statement(statement);
            }

            Statement::Function(func) => {
                self.functions.insert(func.name, Rc::new(func.clone()));
            }
        }
    }

    fn execute_expression(&mut self, expression: &Expression<'source_code>) -> Value {
        match expression {
            Expression::Function(func) => self.execute_function_call(func),
            Expression::BiExpression(expr) => self.execute_bi_expression(expr),

            Expression::Primary(PrimaryExpression::Reference(reference)) => {
                self.scope.find(reference)
            }

            Expression::Primary(PrimaryExpression::IntegerLiteral(integer)) => {
                Value::Integer(*integer)
            }

            Expression::Primary(PrimaryExpression::StringLiteral(str)) => {
                Value::String(str.to_string())
            }

            Expression::Primary(PrimaryExpression::TemplateString{ parts }) => {
                let mut string = String::new();

                for part in parts {
                    match part {
                        TemplateStringExpressionPart::String(str) => {
                            string += str;
                        }

                        TemplateStringExpressionPart::Expression(expression) => {
                            string += &self.execute_expression(expression).to_string();
                        }
                    }
                }

                Value::String(string)
            }
        }
    }

    fn execute_for_statement(&mut self, statement: &ForStatement<'source_code>) -> Value {
        let PrimaryExpression::IntegerLiteral(start) = statement.range.start else {
            panic!("Invalid start");
        };

        let PrimaryExpression::IntegerLiteral(end) = statement.range.end else {
            panic!("Invalid end");
        };

        self.scope = std::mem::take(&mut self.scope).push();

        for x in start..end {
            self.scope.variables.insert(statement.iterator_name, Value::Integer(x));

            for statement in &statement.body {
                self.execute(statement);
            }
        }

        self.scope = std::mem::take(&mut self.scope).pop();

        Value::Null
    }

    fn execute_bi_expression(&mut self, expression: &BiExpression<'source_code>) -> Value {
        let lhs = self.execute_expression(&expression.lhs);
        let rhs = self.execute_expression(&expression.rhs);

        match expression.operator {
            BiOperator::Add => self.execute_expression_add(lhs, rhs),
            BiOperator::Subtract => self.execute_expression_subtract(lhs, rhs),
            BiOperator::Multiply => self.execute_expression_multiply(lhs, rhs),
        }
    }

    fn execute_expression_add(&mut self, lhs: Value, rhs: Value) -> Value {
        match (&lhs, &rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs + rhs),
            (Value::String(lhs), Value::String(rhs)) => Value::String(format!("{lhs}{rhs}")),
            _ => panic!("Invalid operands for add: {lhs:?} and {rhs:?}"),
        }
    }

    fn execute_expression_subtract(&mut self, lhs: Value, rhs: Value) -> Value {
        match (&lhs, &rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs - rhs),
            _ => panic!("Invalid operands for add: {lhs:?} and {rhs:?}"),
        }
    }

    fn execute_expression_multiply(&mut self, lhs: Value, rhs: Value) -> Value {
        match (&lhs, &rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs * rhs),
            _ => panic!("Invalid operands for add: {lhs:?} and {rhs:?}"),
        }
    }

    fn execute_function_call(&mut self, func: &FunctionCallExpression<'source_code>) -> Value {
        let mut arguments = Vec::with_capacity(func.arguments.len());
        for argument in &func.arguments {
            arguments.push(self.execute_expression(argument));
        }

        let name: &str = func.function_identifier.as_ref();

        if let Some(func) = self.functions.get(name).cloned() {
            for statement in &func.body {
                self.execute(statement);
            }
            return Value::Null;
        }

        for builtin_func in Builtin::FUNCTIONS {
            if builtin_func.name == name {
                return (builtin_func.function)(self, arguments);
            }
        }

        println!("Error: Unknown function {name}");
        Value::Null
    }
}