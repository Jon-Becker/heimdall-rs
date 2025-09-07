use std::fmt::Write;

use alloy::primitives::U256;
use heimdall_common::utils::strings::encode_hex_reduced;

use crate::ir::types::{
    BinOp, Block, CallType, Expr, Function, LoadType, Param, ParamLocation, SolidityType, Stmt,
    StoreType, Terminator, UnOp, Visibility,
};

pub struct SolidityEmitter {
    indent_level: usize,
    indent_str: String,
}

impl SolidityEmitter {
    pub fn new() -> Self {
        Self {
            indent_level: 0,
            indent_str: "    ".to_string(),
        }
    }

    pub fn emit(&self, func: &Function) -> String {
        let mut output = String::new();
        self.emit_function(&mut output, func);
        output
    }

    fn emit_function(&self, output: &mut String, func: &Function) {
        // Function signature
        writeln!(
            output,
            "function Function_{:x}(",
            func.selector.unwrap_or(U256::ZERO)
        )
        .unwrap();

        // Parameters
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                write!(output, ", ").unwrap();
            }
            self.emit_param(output, param);
        }
        write!(output, ") ").unwrap();

        // Visibility
        self.emit_visibility(output, &func.visibility);

        // Modifiers
        for modifier in &func.modifiers {
            write!(output, " {}", modifier).unwrap();
        }

        // Returns
        if !func.returns.is_empty() {
            write!(output, " returns (").unwrap();
            for (i, ret_type) in func.returns.iter().enumerate() {
                if i > 0 {
                    write!(output, ", ").unwrap();
                }
                self.emit_type(output, ret_type);
            }
            write!(output, ")").unwrap();
        }

        writeln!(output, " {{").unwrap();

        // Function body
        let mut emitter = SolidityEmitter {
            indent_level: self.indent_level + 1,
            indent_str: self.indent_str.clone(),
        };

        for block in &func.blocks {
            emitter.emit_block(output, block);
        }

        writeln!(output, "}}").unwrap();
    }

    fn emit_block(&self, output: &mut String, block: &Block) {
        // Emit label if present
        if let Some(label) = &block.label {
            writeln!(output, "{}// Label_{:x}:", self.indent(), label.0).unwrap();
        }

        // Emit statements
        for stmt in &block.stmts {
            self.emit_stmt(output, stmt);
        }

        // Emit terminator
        if let Some(term) = &block.terminator {
            self.emit_terminator(output, term);
        }
    }

    fn emit_stmt(&self, output: &mut String, stmt: &Stmt) {
        write!(output, "{}", self.indent()).unwrap();

        match stmt {
            Stmt::Assign(var, expr) => {
                write!(output, "{} = ", var).unwrap();
                self.emit_expr(output, expr, 0);
                writeln!(output, ";").unwrap();
            }
            Stmt::Store(store_type, slot, value) => {
                match store_type {
                    StoreType::Memory => {
                        write!(output, "memory[").unwrap();
                        self.emit_expr(output, slot, 0);
                        write!(output, "] = ").unwrap();
                    }
                    StoreType::Storage => {
                        write!(output, "storage[").unwrap();
                        self.emit_expr(output, slot, 0);
                        write!(output, "] = ").unwrap();
                    }
                    StoreType::Transient => {
                        write!(output, "transient[").unwrap();
                        self.emit_expr(output, slot, 0);
                        write!(output, "] = ").unwrap();
                    }
                }
                self.emit_expr(output, value, 0);
                writeln!(output, ";").unwrap();
            }
            Stmt::If(cond, then_block, else_block) => {
                write!(output, "if (").unwrap();
                self.emit_expr(output, cond, 0);
                writeln!(output, ") {{").unwrap();

                let mut inner = SolidityEmitter {
                    indent_level: self.indent_level + 1,
                    indent_str: self.indent_str.clone(),
                };
                inner.emit_block(output, then_block);

                write!(output, "{}}}", self.indent()).unwrap();

                if let Some(else_b) = else_block {
                    writeln!(output, " else {{").unwrap();
                    inner.emit_block(output, else_b);
                    write!(output, "{}}}", self.indent()).unwrap();
                }
                writeln!(output).unwrap();
            }
            Stmt::While(cond, body) => {
                write!(output, "while (").unwrap();
                self.emit_expr(output, cond, 0);
                writeln!(output, ") {{").unwrap();

                let mut inner = SolidityEmitter {
                    indent_level: self.indent_level + 1,
                    indent_str: self.indent_str.clone(),
                };
                inner.emit_block(output, body);

                writeln!(output, "{}}}", self.indent()).unwrap();
            }
            Stmt::Return(exprs) => {
                write!(output, "return").unwrap();
                if !exprs.is_empty() {
                    write!(output, " ").unwrap();
                    for (i, expr) in exprs.iter().enumerate() {
                        if i > 0 {
                            write!(output, ", ").unwrap();
                        }
                        self.emit_expr(output, expr, 0);
                    }
                }
                writeln!(output, ";").unwrap();
            }
            Stmt::Revert(exprs) => {
                write!(output, "revert").unwrap();
                if !exprs.is_empty() {
                    write!(output, "(").unwrap();
                    for (i, expr) in exprs.iter().enumerate() {
                        if i > 0 {
                            write!(output, ", ").unwrap();
                        }
                        self.emit_expr(output, expr, 0);
                    }
                    write!(output, ")").unwrap();
                }
                writeln!(output, ";").unwrap();
            }
            Stmt::Jump(label) => {
                writeln!(output, "goto Label_{:x};", label.0).unwrap();
            }
            Stmt::Call(call_type) => {
                self.emit_call(output, call_type);
                writeln!(output, ";").unwrap();
            }
            Stmt::Log(topic_count, topics) => {
                write!(output, "emit Log{}(", topic_count).unwrap();
                for (i, topic) in topics.iter().enumerate() {
                    if i > 0 {
                        write!(output, ", ").unwrap();
                    }
                    self.emit_expr(output, topic, 0);
                }
                writeln!(output, ");").unwrap();
            }
        }
    }

    fn emit_terminator(&self, output: &mut String, term: &Terminator) {
        write!(output, "{}", self.indent()).unwrap();

        match term {
            Terminator::Return(exprs) => {
                write!(output, "return").unwrap();
                if !exprs.is_empty() {
                    write!(output, " ").unwrap();
                    for (i, expr) in exprs.iter().enumerate() {
                        if i > 0 {
                            write!(output, ", ").unwrap();
                        }
                        self.emit_expr(output, expr, 0);
                    }
                }
                writeln!(output, ";").unwrap();
            }
            Terminator::Revert(exprs) => {
                write!(output, "revert").unwrap();
                if !exprs.is_empty() {
                    write!(output, "(").unwrap();
                    for (i, expr) in exprs.iter().enumerate() {
                        if i > 0 {
                            write!(output, ", ").unwrap();
                        }
                        self.emit_expr(output, expr, 0);
                    }
                    write!(output, ")").unwrap();
                }
                writeln!(output, ";").unwrap();
            }
            Terminator::Jump(label) => {
                writeln!(output, "goto Label_{:x};", label.0).unwrap();
            }
            Terminator::ConditionalJump(cond, true_label, false_label) => {
                write!(output, "if (").unwrap();
                self.emit_expr(output, cond, 0);
                write!(output, ") goto Label_{:x}", true_label.0).unwrap();
                if let Some(false_l) = false_label {
                    write!(output, " else goto Label_{:x}", false_l.0).unwrap();
                }
                writeln!(output, ";").unwrap();
            }
            Terminator::Stop => {
                writeln!(output, "stop();").unwrap();
            }
        }
    }

    fn emit_expr(&self, output: &mut String, expr: &Expr, parent_precedence: u8) {
        match expr {
            Expr::Const(val) => {
                write!(output, "{}", encode_hex_reduced(*val)).unwrap();
            }
            Expr::Var(name) => {
                write!(output, "{}", name).unwrap();
            }
            Expr::BinOp(op, left, right) => {
                let precedence = op.precedence();
                let needs_parens = precedence < parent_precedence;

                if needs_parens {
                    write!(output, "(").unwrap();
                }

                self.emit_expr(output, left, precedence);
                write!(output, " {} ", op).unwrap();
                self.emit_expr(output, right, precedence + if op.is_associative() { 0 } else { 1 });

                if needs_parens {
                    write!(output, ")").unwrap();
                }
            }
            Expr::UnOp(op, operand) => {
                match op {
                    UnOp::Not => write!(output, "~").unwrap(),
                    UnOp::IsZero => write!(output, "!").unwrap(),
                    UnOp::Neg => write!(output, "-").unwrap(),
                }
                self.emit_expr(output, operand, 15); // Unary has high precedence
            }
            Expr::Call(call_type, _args) => {
                self.emit_call(output, call_type);
            }
            Expr::Load(load_type, addr) => {
                match load_type {
                    LoadType::Memory => write!(output, "memory[").unwrap(),
                    LoadType::Storage => write!(output, "storage[").unwrap(),
                    LoadType::Calldata => write!(output, "calldata[").unwrap(),
                    LoadType::Transient => write!(output, "transient[").unwrap(),
                }
                self.emit_expr(output, addr, 0);
                write!(output, "]").unwrap();
            }
            Expr::Cast(ty, inner) => {
                self.emit_type(output, ty);
                write!(output, "(").unwrap();
                self.emit_expr(output, inner, 0);
                write!(output, ")").unwrap();
            }
            Expr::Ternary(cond, then_expr, else_expr) => {
                let needs_parens = parent_precedence > 3;
                if needs_parens {
                    write!(output, "(").unwrap();
                }
                self.emit_expr(output, cond, 4);
                write!(output, " ? ").unwrap();
                self.emit_expr(output, then_expr, 3);
                write!(output, " : ").unwrap();
                self.emit_expr(output, else_expr, 3);
                if needs_parens {
                    write!(output, ")").unwrap();
                }
            }
        }
    }

    fn emit_call(&self, output: &mut String, call_type: &CallType) {
        match call_type {
            CallType::Call(addr, value, args) => {
                self.emit_expr(output, addr, 0);
                write!(output, ".call{{value: ").unwrap();
                self.emit_expr(output, value, 0);
                write!(output, "}}(").unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(output, ", ").unwrap();
                    }
                    self.emit_expr(output, arg, 0);
                }
                write!(output, ")").unwrap();
            }
            CallType::DelegateCall(addr, args) => {
                self.emit_expr(output, addr, 0);
                write!(output, ".delegatecall(").unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(output, ", ").unwrap();
                    }
                    self.emit_expr(output, arg, 0);
                }
                write!(output, ")").unwrap();
            }
            CallType::StaticCall(addr, args) => {
                self.emit_expr(output, addr, 0);
                write!(output, ".staticcall(").unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(output, ", ").unwrap();
                    }
                    self.emit_expr(output, arg, 0);
                }
                write!(output, ")").unwrap();
            }
            CallType::Create(value, code) => {
                write!(output, "new Contract{{value: ").unwrap();
                self.emit_expr(output, value, 0);
                write!(output, "}}(").unwrap();
                self.emit_expr(output, code, 0);
                write!(output, ")").unwrap();
            }
            CallType::Create2(value, code, salt) => {
                write!(output, "new Contract{{value: ").unwrap();
                self.emit_expr(output, value, 0);
                write!(output, ", salt: ").unwrap();
                self.emit_expr(output, salt, 0);
                write!(output, "}}(").unwrap();
                self.emit_expr(output, code, 0);
                write!(output, ")").unwrap();
            }
        }
    }

    fn emit_param(&self, output: &mut String, param: &Param) {
        self.emit_type(output, &param.ty);
        match param.location {
            ParamLocation::Memory => write!(output, " memory").unwrap(),
            ParamLocation::Calldata => write!(output, " calldata").unwrap(),
            ParamLocation::Storage => write!(output, " storage").unwrap(),
        }
        write!(output, " {}", param.name).unwrap();
    }

    fn emit_type(&self, output: &mut String, ty: &SolidityType) {
        match ty {
            SolidityType::Uint(bits) => write!(output, "uint{}", bits).unwrap(),
            SolidityType::Int(bits) => write!(output, "int{}", bits).unwrap(),
            SolidityType::Address => write!(output, "address").unwrap(),
            SolidityType::Bool => write!(output, "bool").unwrap(),
            SolidityType::Bytes(size) => write!(output, "bytes{}", size).unwrap(),
            SolidityType::BytesDynamic => write!(output, "bytes").unwrap(),
            SolidityType::String => write!(output, "string").unwrap(),
        }
    }

    fn emit_visibility(&self, output: &mut String, vis: &Visibility) {
        match vis {
            Visibility::Public => write!(output, "public").unwrap(),
            Visibility::External => write!(output, "external").unwrap(),
            Visibility::Internal => write!(output, "internal").unwrap(),
            Visibility::Private => write!(output, "private").unwrap(),
        }
    }

    fn indent(&self) -> String {
        self.indent_str.repeat(self.indent_level)
    }
}

impl Default for SolidityEmitter {
    fn default() -> Self {
        Self::new()
    }
}