use anyhow::Result;
use std::fmt::Display;
use types::type_checker::TypeChecker;

use bytecode::{ByteCode, Value};
use parser::structs::{BinOpType, BlockSeq, Decl, Expr, IfElseData, UnOpType};

pub struct Compiler {
    program: BlockSeq,
}

#[derive(Debug, PartialEq)]
pub struct CompileError {
    msg: String,
}

impl CompileError {
    pub fn new(err: &str) -> CompileError {
        CompileError {
            msg: err.to_owned(),
        }
    }
}

impl Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[CompileError] -  {}", self.msg)
    }
}

impl std::error::Error for CompileError {}

impl Compiler {
    pub fn new(program: BlockSeq) -> Compiler {
        Compiler { program }
    }

    fn compile_unop(
        op: &UnOpType,
        expr: &Expr,
        arr: &mut Vec<ByteCode>,
    ) -> Result<(), CompileError> {
        Compiler::compile_expr(expr, false, arr)?;
        match op {
            UnOpType::Negate => arr.push(ByteCode::UNOP(bytecode::UnOp::Neg)),
            UnOpType::Not => arr.push(ByteCode::UNOP(bytecode::UnOp::Not)),
        }
        Ok(())
    }

    // TODO: how to do type checking here?
    // Distinct phase before compilation is reached? Assign types to all expressions
    fn compile_binop(
        op: &BinOpType,
        lhs: &Expr,
        rhs: &Expr,
        arr: &mut Vec<ByteCode>,
    ) -> Result<(), CompileError> {
        Compiler::compile_expr(lhs, false, arr)?;
        Compiler::compile_expr(rhs, false, arr)?;
        match op {
            BinOpType::Add => arr.push(ByteCode::BINOP(bytecode::BinOp::Add)),
            BinOpType::Mul => arr.push(ByteCode::BINOP(bytecode::BinOp::Mul)),
            BinOpType::Div => arr.push(ByteCode::BINOP(bytecode::BinOp::Div)),
            BinOpType::Sub => arr.push(ByteCode::BINOP(bytecode::BinOp::Sub)),
        }

        Ok(())
    }

    pub fn compile_expr(
        expr: &Expr,
        as_stmt: bool,
        arr: &mut Vec<ByteCode>,
    ) -> Result<(), CompileError> {
        match expr {
            Expr::Integer(val) => arr.push(ByteCode::ldc(*val)),
            Expr::Float(val) => arr.push(ByteCode::ldc(*val)),
            Expr::Bool(val) => arr.push(ByteCode::ldc(*val)),
            Expr::BinOpExpr(op, lhs, rhs) => {
                Compiler::compile_binop(op, lhs, rhs, arr)?;
            }
            Expr::UnOpExpr(op, expr) => {
                Compiler::compile_unop(op, expr, arr)?;
            }
            // Load symbol
            Expr::Symbol(sym) => {
                arr.push(ByteCode::LD(sym.to_string()));
            }
            Expr::BlockExpr(blk) => {
                Compiler::compile_block(blk, as_stmt, arr)?;
            }
            Expr::IfElseExpr(if_else) => Compiler::compile_if_else(if_else, false, arr)?,
        }

        Ok(())
    }

    fn compile_assign(
        ident: &String,
        expr: &Expr,
        arr: &mut Vec<ByteCode>,
    ) -> Result<(), CompileError> {
        Compiler::compile_expr(expr, false, arr)?;

        let assign = ByteCode::ASSIGN(ident.to_owned());
        arr.push(assign);

        // Load unit after stmt to be consistent with popping after every stmt
        arr.push(ByteCode::LDC(Value::Unit));

        Ok(())
    }

    /// Compile block appropriately based on whether it is none-like and whether we intend to compile as expr or stmt
    fn compile_block(
        blk: &BlockSeq,
        as_stmt: bool,
        arr: &mut Vec<ByteCode>,
    ) -> Result<(), CompileError> {
        let decls = &blk.decls;
        let syms = &blk.symbols;

        if !syms.is_empty() {
            arr.push(ByteCode::ENTERSCOPE(syms.clone()));
        }

        for decl in decls {
            Compiler::compile_decl(decl, arr)?;
            // pop result of statements - need to ensure all stmts produce something (either Unit or something else)
            arr.push(ByteCode::POP);
        }

        // Handle expr
        if let Some(expr) = &blk.last_expr {
            Compiler::compile_expr(expr.as_ref(), false, arr)?;
        }

        if !syms.is_empty() {
            arr.push(ByteCode::EXITSCOPE);
        }

        // does not produce value AND treated as stmt: push unit so pop does not underflow
        if Compiler::blk_produces_nothing(blk) && as_stmt {
            arr.push(ByteCode::ldc(Value::Unit));
        }

        Ok(())
    }

    // blk is_none_like if: last_expr is none, or last_expr is a block and the block is none_like
    // none_like meaning the last expr actually leaves nothing on the stack
    fn blk_produces_nothing(blk: &BlockSeq) -> bool {
        if let Some(expr) = &blk.last_expr {
            if let Expr::BlockExpr(seq) = expr.as_ref() {
                Compiler::blk_produces_nothing(seq)
            } else {
                // have last expr and it's not a block: not none like
                false
            }
        } else {
            // no last expr: is none like
            true
        }
    }

    fn compile_decl(decl: &Decl, arr: &mut Vec<ByteCode>) -> Result<(), CompileError> {
        match decl {
            Decl::ExprStmt(expr) => {
                // if let Expr::BlockExpr(seq) = expr {
                //     Compiler::compile_block(seq, true, arr)?;
                // } else {
                //     Compiler::compile_expr(expr, true, arr)?;
                // }

                Compiler::compile_expr(expr, true, arr)?;
            }
            Decl::LetStmt(stmt) => {
                Compiler::compile_assign(&stmt.ident, &stmt.expr, arr)?;
            }
            Decl::Assign(stmt) => {
                Compiler::compile_assign(&stmt.ident, &stmt.expr, arr)?;
            }
        };

        Ok(())
    }

    /// Compile if_else as statement or as expr - changes how blocks are compiled
    fn compile_if_else(
        if_else: &IfElseData,
        as_stmt: bool,
        arr: &mut Vec<ByteCode>,
    ) -> Result<(), CompileError> {
        dbg!("COMPILE IF ELSE");
        Compiler::compile_expr(&if_else.cond, false, arr)?;

        let jof_idx = arr.len();
        arr.push(ByteCode::JOF(0));

        Compiler::compile_block(&if_else.if_blk, as_stmt, arr)?;

        let goto_idx = arr.len();
        arr.push(ByteCode::GOTO(0));

        let jof_addr = arr.len(); // jump to after the GOTO

        if let Some(else_blk) = &if_else.else_blk {
            Compiler::compile_block(else_blk, as_stmt, arr)?;
        }

        let goto_addr = arr.len(); // jump to after else blk

        // set JOF arg
        if let Some(ByteCode::JOF(idx)) = arr.get_mut(jof_idx) {
            *idx = jof_addr;
            // if let ByteCode::JOF(idx) = inst {
            //     *idx = jof_addr;
            // }
        }

        if let Some(ByteCode::GOTO(idx)) = arr.get_mut(goto_idx) {
            // if let ByteCode::GOTO(idx) = inst {
            //     *idx = goto_addr;
            // }
            *idx = goto_addr;
        }
        Ok(())
    }

    pub fn compile(self) -> anyhow::Result<Vec<ByteCode>, CompileError> {
        let mut bytecode: Vec<ByteCode> = vec![];
        Compiler::compile_block(&self.program, false, &mut bytecode)?;
        bytecode.push(ByteCode::DONE);

        Ok(bytecode)
    }
}

/// Takes in a string and returns compiled bytecode or errors
pub fn compile_from_string(inp: &str, type_check: bool) -> Result<Vec<ByteCode>> {
    let parser = parser::Parser::new_from_string(inp);
    let program = parser.parse()?;

    if type_check {
        TypeChecker::new(&program).type_check()?;
    }

    let compiler = Compiler::new(program);
    Ok(compiler.compile()?)
}

#[cfg(test)]
mod tests {

    use bytecode::ByteCode;
    use bytecode::ByteCode::*;
    use bytecode::Value::*;
    use parser::Parser;

    use super::Compiler;

    fn exp_compile_str(inp: &str) -> Vec<ByteCode> {
        let parser = Parser::new_from_string(inp);
        let parsed = parser.parse().expect("Should parse");
        dbg!("parsed:", &parsed);
        let comp = Compiler::new(parsed);
        comp.compile().expect("Should compile")
    }

    fn test_comp(inp: &str, exp: Vec<ByteCode>) {
        let res = exp_compile_str(inp);
        dbg!(&res);
        assert_eq!(res, exp);
    }

    #[test]
    fn test_compile_simple() {
        let res = exp_compile_str("42;");
        assert_eq!(res, vec![ByteCode::ldc(42), POP, DONE]);

        let res = exp_compile_str("42; 45; 30");
        assert_eq!(
            res,
            vec![
                ByteCode::ldc(42),
                POP,
                ByteCode::ldc(45),
                POP,
                ByteCode::ldc(30),
                DONE
            ]
        );

        let res = exp_compile_str("42; true; 2.36;");
        assert_eq!(
            res,
            vec![
                ByteCode::ldc(42),
                POP,
                ByteCode::ldc(true),
                POP,
                ByteCode::ldc(2.36),
                POP,
                DONE
            ]
        )
    }

    #[test]
    fn test_compile_binop() {
        let res = exp_compile_str("2+3*2-4;");
        let exp = vec![
            LDC(Int(2)),
            LDC(Int(3)),
            LDC(Int(2)),
            BINOP(bytecode::BinOp::Mul),
            BINOP(bytecode::BinOp::Add),
            LDC(Int(4)),
            BINOP(bytecode::BinOp::Sub),
            POP,
            DONE,
        ];

        assert_eq!(res, exp);

        let res = exp_compile_str("2+3*4-5/5");

        let exp = [
            LDC(Int(2)),
            LDC(Int(3)),
            LDC(Int(4)),
            BINOP(bytecode::BinOp::Mul),
            BINOP(bytecode::BinOp::Add),
            LDC(Int(5)),
            LDC(Int(5)),
            BINOP(bytecode::BinOp::Div),
            BINOP(bytecode::BinOp::Sub),
            DONE,
        ];

        assert_eq!(res, exp);
    }

    #[test]
    fn test_compile_let() {
        let res = exp_compile_str("let x = 2;");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            EXITSCOPE,
            DONE,
        ];

        assert_eq!(res, exp);

        // stmt last
        let res = exp_compile_str("let x = 2; let y = 3; ");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string(), "y".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            LDC(Int(3)),
            ASSIGN("y".to_string()),
            LDC(Unit),
            POP,
            EXITSCOPE,
            DONE,
        ];

        assert_eq!(res, exp);

        // many
        let res = exp_compile_str("let x = 2; let y = 3; 40");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string(), "y".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            LDC(Int(3)),
            ASSIGN("y".to_string()),
            LDC(Unit),
            POP,
            LDC(Int(40)),
            EXITSCOPE,
            DONE,
        ];

        assert_eq!(res, exp);
    }

    #[test]
    fn test_compile_sym() {
        let res = exp_compile_str("let x = 2; -x+2;");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            LD("x".to_string()),
            UNOP(bytecode::UnOp::Neg),
            LDC(Int(2)),
            BINOP(bytecode::BinOp::Add),
            POP,
            EXITSCOPE,
            DONE,
        ];
        assert_eq!(res, exp);

        let res = exp_compile_str("let x = 2; let y = x; x*5+2");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string(), "y".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            LD("x".to_string()),
            ASSIGN("y".to_string()),
            LDC(Unit),
            POP,
            LD("x".to_string()),
            LDC(Int(5)),
            BINOP(bytecode::BinOp::Mul),
            LDC(Int(2)),
            BINOP(bytecode::BinOp::Add),
            EXITSCOPE,
            DONE,
        ];

        assert_eq!(res, exp);
    }

    #[test]
    fn test_compile_not() {
        let res = exp_compile_str("!true");
        let exp = [LDC(Bool(true)), UNOP(bytecode::UnOp::Not), DONE];
        assert_eq!(res, exp);

        let res = exp_compile_str("!!false");
        let exp = [
            LDC(Bool(false)),
            UNOP(bytecode::UnOp::Not),
            UNOP(bytecode::UnOp::Not),
            DONE,
        ];
        assert_eq!(res, exp);

        let res = exp_compile_str("!!!true;");
        let exp = [
            LDC(Bool(true)),
            UNOP(bytecode::UnOp::Not),
            UNOP(bytecode::UnOp::Not),
            UNOP(bytecode::UnOp::Not),
            POP,
            DONE,
        ];
        assert_eq!(res, exp);
    }

    #[test]
    fn test_compile_assign() {
        let res = exp_compile_str("let x = 2; x = 3;");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            LDC(Int(3)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            EXITSCOPE,
            DONE,
        ];
        assert_eq!(res, exp);

        // diff types
        let res = exp_compile_str("let x = 2; x = true;");
        let exp = vec![
            ENTERSCOPE(vec!["x".to_string()]),
            LDC(Int(2)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            LDC(Bool(true)),
            ASSIGN("x".to_string()),
            LDC(Unit),
            POP,
            EXITSCOPE,
            DONE,
        ];
        assert_eq!(res, exp);
    }

    use bytecode::Value::*;
    #[test]
    fn test_compile_blk_simple() {
        let t = "{ 2 }";
        let exp = vec![ByteCode::ldc(2), DONE];
        test_comp(t, exp);

        let t = "{ 2; 3 }";
        let exp = vec![ByteCode::ldc(2), ByteCode::POP, ByteCode::ldc(3), DONE];
        test_comp(t, exp);

        let t = "{ 2; 3; }";
        let exp = vec![
            ByteCode::ldc(2),
            ByteCode::POP,
            ByteCode::ldc(3),
            ByteCode::POP,
            DONE,
        ];
        test_comp(t, exp);

        let t = "{ 2; 3; 4 }";
        let exp = vec![
            ByteCode::ldc(2),
            ByteCode::POP,
            ByteCode::ldc(3),
            ByteCode::POP,
            ByteCode::ldc(4),
            DONE,
        ];
        test_comp(t, exp);

        // like doing just 4;
        let t = "{ 2; 3; 4 };";
        let exp = vec![
            ByteCode::ldc(2),
            ByteCode::POP,
            ByteCode::ldc(3),
            ByteCode::POP,
            ByteCode::ldc(4),
            ByteCode::POP,
            DONE,
        ];
        test_comp(t, exp);

        // wrong
        let t = "{ 2; 3; 4; };";
        let exp = vec![
            ByteCode::ldc(2),
            ByteCode::POP,
            ByteCode::ldc(3),
            ByteCode::POP,
            ByteCode::ldc(4),
            ByteCode::POP,
            ByteCode::ldc(Unit),
            ByteCode::POP,
            DONE,
        ];
        test_comp(t, exp);
    }

    #[test]
    fn test_compile_blk_cases() {
        test_comp("{ 2 }", vec![ByteCode::ldc(2), DONE]);
        test_comp("{ 2; }", vec![ByteCode::ldc(2), POP, DONE]);

        // since we pop after every stmt, if the block ends in expr we just rely on that
        test_comp("{ 2 };", vec![ByteCode::ldc(2), POP, DONE]);

        // we pop after every stmt, but since this blk has no last expr we push unit before blk ends so the pop doesn't
        // underflow
        test_comp(
            "{ 2; };",
            vec![ByteCode::ldc(2), POP, ByteCode::ldc(Unit), POP, DONE],
        );
    }

    #[test]
    fn test_compile_blk_let() {
        let t = r"
        let x = 2;
        {
            let y = 3;
            x+y
        }
        ";
        test_comp(
            t,
            vec![
                ENTERSCOPE(vec!["x".to_string()]),
                ByteCode::ldc(2),
                ASSIGN("x".to_string()),
                ByteCode::ldc(Unit),
                POP,
                ENTERSCOPE(vec!["y".to_string()]),
                LDC(Int(3)),
                ASSIGN("y".to_string()),
                LDC(Unit),
                POP,
                LD("x".to_string()),
                LD("y".to_string()),
                ByteCode::binop("+"),
                EXITSCOPE,
                EXITSCOPE,
                DONE,
            ],
        );

        let t = r"
        let x = 2; { {2+2;} };
        ";

        test_comp(
            t,
            vec![
                ENTERSCOPE(vec!["x".to_string()]),
                ByteCode::ldc(2),
                ASSIGN("x".to_string()),
                LDC(Unit),
                POP,
                LDC(Int(2)),
                LDC(Int(2)),
                ByteCode::binop("+"),
                POP,
                LDC(Unit),
                POP,
                EXITSCOPE,
                DONE,
            ],
        );

        // nested none-like
        let t = r"
        let x = 2; { 

            {
                {
                    2+2;
                }
            } 
        
        };
        ";

        test_comp(
            t,
            vec![
                ENTERSCOPE(vec!["x".to_string()]),
                ByteCode::ldc(2),
                ASSIGN("x".to_string()),
                LDC(Unit),
                POP,
                LDC(Int(2)),
                LDC(Int(2)),
                ByteCode::binop("+"),
                POP,
                LDC(Unit),
                POP,
                EXITSCOPE,
                DONE,
            ],
        );
    }

    #[test]
    fn test_compile_if() {
        let t = r"
        if (true) {
            10;
            2
        } else {
            20;
            3
        }
        ";

        // I just copied this from the print
        let exp = vec![
            LDC(Bool(true)),
            JOF(6),
            LDC(Int(10)),
            POP,
            LDC(Int(2)),
            GOTO(9),
            LDC(Int(20)),
            POP,
            LDC(Int(3)),
            DONE,
        ];
        test_comp(t, exp);
    }
}
