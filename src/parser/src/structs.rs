use std::fmt::{Display, Formatter};
use std::rc::Rc;

use lexer::Token;

#[derive(Debug, Clone)]
pub enum BinOpType {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    LogicalEq,
    LogicalAnd,
    LogicalOr,
}

impl BinOpType {
    pub fn from_token(token: &Token) -> Result<BinOpType, ParseError> {
        match token {
            Token::Plus => Ok(Self::Add),
            Token::Minus => Ok(Self::Sub),
            Token::Star => Ok(Self::Mul),
            Token::Slash => Ok(Self::Div),
            Token::Gt => Ok(Self::Gt),
            Token::Lt => Ok(Self::Lt),
            Token::LogEq => Ok(Self::LogicalEq),
            Token::LogAnd => Ok(Self::LogicalAnd),
            Token::LogOr => Ok(Self::LogicalOr),
            _ => Err(ParseError::new(&format!(
                "Expected infix operator but got: {}",
                token
            ))),
        }
    }
}

impl Display for BinOpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chr = match self {
            BinOpType::Add => "+",
            BinOpType::Sub => "-",
            BinOpType::Mul => "*",
            BinOpType::Div => "/",
            BinOpType::Lt => "<",
            BinOpType::Gt => ">",
            BinOpType::LogicalEq => "==",
            BinOpType::LogicalAnd => "&&",
            BinOpType::LogicalOr => "||",
        };
        write!(f, "{}", chr)
    }
}

#[derive(Debug, Clone)]
pub enum UnOpType {
    Negate,
    Not,
}

impl Display for UnOpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chr = match self {
            Self::Negate => "-",
            Self::Not => "!",
        };

        write!(f, "{}", chr)
    }
}

// Function call
#[derive(Debug, Clone)]
pub struct FnCallData {
    pub name: String,
    pub args: Vec<Expr>,
}

impl Display for FnCallData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args: Vec<String> = self.args.iter().map(|x| x.to_string()).collect();
        let args = args.join(",");

        let s = format!("{}({})", self.name, args);

        write!(f, "{}", s)
    }
}

// Different from bytecode Value because values on op stack might be different (e.g fn call)
#[derive(Debug, Clone)]
pub enum Expr {
    Symbol(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    StringLiteral(String),
    UnOpExpr(UnOpType, Box<Expr>),
    BinOpExpr(BinOpType, Box<Expr>, Box<Expr>),
    BlockExpr(BlockSeq), // expr can be a block
    IfElseExpr(Box<IfElseData>),
    FnCallExpr(FnCallData),
    SpawnExpr(FnCallData),
    // Because join can return something so must be able to assign to it
    // String is the symbol of the thread id to join
    JoinExpr(String),
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Expr::Integer(val) => val.to_string(),
            Expr::Float(val) => val.to_string(),
            Expr::Bool(val) => val.to_string(),
            Expr::UnOpExpr(op, expr) => {
                format!("({}{})", op, expr)
            }
            Expr::BinOpExpr(op, lhs, rhs) => {
                format!("({}{}{})", lhs, op, rhs)
            }
            Expr::Symbol(val) => val.to_string(),
            Expr::BlockExpr(seq) => format!("{{ {} }}", seq),
            // Expr::BlockExpr(seq) => seq.to_string(),
            Expr::IfElseExpr(expr) => expr.to_string(),
            Expr::FnCallExpr(expr) => expr.to_string(),
            Expr::SpawnExpr(expr) => format!("spawn {}", expr),
            Expr::JoinExpr(sym) => format!("join {}", sym),
            Expr::StringLiteral(str) => str.to_string(),
        };

        write!(f, "{}", string)
    }
}

#[derive(Debug, Clone)]
pub struct LetStmtData {
    pub ident: String,
    pub expr: Expr,
    pub type_ann: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct AssignStmtData {
    pub ident: String,
    pub expr: Expr,
}

impl Display for LetStmtData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = if let Some(ty) = &self.type_ann {
            format!("let {} : {} = {}", self.ident, ty, self.expr)
        } else {
            format!("let {} = {}", self.ident, self.expr)
        };

        write!(f, "{}", string)
    }
}

impl Display for AssignStmtData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {}", self.ident, self.expr)
    }
}

#[derive(Debug, Clone)]
pub struct IfElseData {
    pub cond: Expr,
    pub if_blk: BlockSeq,
    pub else_blk: Option<BlockSeq>,
}

impl Display for IfElseData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = format!("if {} {{ {} }}", self.cond, self.if_blk);
        if let Some(ref else_blk) = self.else_blk {
            s.push(' ');
            s.push_str(&format!("else {{ {} }}", else_blk));
        }

        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub struct LoopData {
    pub cond: Option<Expr>,
    pub body: BlockSeq,
}

impl Display for LoopData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cond_str = self
            .cond
            .as_ref()
            .map(|x| x.to_string())
            .unwrap_or("".to_string());
        let body_str = format!("{{ {} }}", self.body);
        write!(f, "loop {} {}", cond_str, body_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
// function parameter
pub struct FnParam {
    pub name: String,
    pub type_ann: Option<Type>,
}

impl Display for FnParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let param_str = if let Some(ty) = &self.type_ann {
            format!("{}:{}", self.name, ty)
        } else {
            self.name.to_string()
        };

        write!(f, "{}", param_str)
    }
}

// Fn Decl
#[derive(Debug, Clone)]
pub struct FnDeclData {
    pub name: String,
    pub params: Vec<FnParam>,
    pub ret_type: Type,
    pub body: BlockSeq,
}

impl Display for FnDeclData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let params: Vec<String> = self.params.iter().map(|x| x.to_string()).collect();
        let params = params.join(", ");

        let ret_type_str = if self.ret_type.eq(&Type::Unit) {
            " ".to_string()
        } else {
            format!(" -> {} ", self.ret_type)
        };

        let s = format!(
            "fn {} ({}){}{{ {} }}",
            self.name, params, ret_type_str, self.body
        );
        write!(f, "{}", s)
    }
}

// Later: LetStmt, IfStmt, FnDef, etc.
#[derive(Debug, Clone)]
pub enum Decl {
    LetStmt(LetStmtData),
    AssignStmt(AssignStmtData),
    ExprStmt(Expr),
    // if with no else should only be stmt. use same struct because compilation is very similar to if-else
    IfOnlyStmt(IfElseData),
    // loop is always a stmt (for now)
    LoopStmt(LoopData),
    FnDeclStmt(FnDeclData),
    // only inside loop
    BreakStmt,
    // only inside fn
    ReturnStmt(Option<Expr>),
    // wait sem; - stmt only
    WaitStmt(String),
    // post sem; - stmt only
    PostStmt(String),
    // yield; - no args
    YieldStmt,
}

impl Decl {
    // Need to clone so we can re-use in pratt parser loop
    // Reasoning: parsing won't take most of the runtime
    pub fn to_expr(&self) -> Result<Expr, ParseError> {
        // Decls that return parse error will always be treated as statements
        match self {
            Self::LetStmt(ref stmt) => {
                Err(ParseError::new(&format!("'{}' is not an expression", stmt)))
            }
            Self::AssignStmt(ref stmt) => {
                Err(ParseError::new(&format!("'{}' is not an expression", stmt)))
            }
            Self::IfOnlyStmt(_) => Err(ParseError::new(
                "if without else branch is not an expression",
            )),
            Self::FnDeclStmt(_) => {
                Err(ParseError::new("Function declaration is not an expression"))
            }
            Self::LoopStmt(_) => Err(ParseError::new("loop is not an expression")),
            Self::BreakStmt => Err(ParseError::new("break is not an expression")),
            Self::ReturnStmt(_) => Err(ParseError::new("return is not an expression")),
            Self::WaitStmt(_) => Err(ParseError::new("wait is not an expression")),
            Self::PostStmt(_) => Err(ParseError::new("post is not an expression")),
            Self::YieldStmt => Err(ParseError::new("yield is not an expression")),
            Self::ExprStmt(expr) => Ok(expr.clone()),
        }
    }

    pub fn to_block(&self) -> Result<BlockSeq, ParseError> {
        if let Self::ExprStmt(Expr::BlockExpr(seq)) = &self {
            return Ok(seq.clone());
        }

        let e = format!("Expected block but got '{}'", self);
        Err(ParseError::new(&e))
    }

    /// Returns true if this Decl has to be treated as a stmt, but has no semicolon terminating
    // TODO: Add function decls later
    pub fn is_stmt_with_no_semi(&self) -> bool {
        matches!(self, Self::IfOnlyStmt(_))
    }
}

impl Display for Decl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Decl::ExprStmt(expr) => expr.to_string(),
            Decl::LetStmt(stmt) => stmt.to_string(),
            Decl::AssignStmt(stmt) => stmt.to_string(),
            Decl::IfOnlyStmt(expr) => expr.to_string(),
            Decl::LoopStmt(lp) => lp.to_string(),
            Decl::BreakStmt => Token::Break.to_string(),
            Decl::FnDeclStmt(fn_decl) => fn_decl.to_string(),
            Decl::ReturnStmt(expr) => {
                let str = expr
                    .clone()
                    .map(|x| x.to_string())
                    .unwrap_or(String::from(""));
                let str = if str.is_empty() {
                    str
                } else {
                    format!(" {}", str)
                };
                format!("{}{}", Token::Return, str)
            }
            Decl::WaitStmt(sym) => format!("wait {}", sym),
            Decl::PostStmt(sym) => format!("post {}", sym),
            Decl::YieldStmt => "yield".to_string(),
        };

        write!(f, "{}", string)
    }
}

// Last expression is value of program semantics (else Unit type)
// Program is either one declaration or a sequence of declarations with optional last expression
#[derive(Debug, Clone)]
pub struct BlockSeq {
    pub decls: Vec<Decl>,
    pub last_expr: Option<Rc<Expr>>,
    // List of top level uninitialised symbols (variable/func declarations)
    pub symbols: Vec<String>,
}

impl Display for BlockSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let decls = self
            .decls
            .iter()
            .map(|d| d.to_string() + ";")
            .collect::<String>();
        let expr = match &self.last_expr {
            Some(expr) => expr.to_string(),
            None => String::from(""),
        };

        write!(f, "{}{}", decls, expr)
    }
}

#[derive(Debug, PartialEq)]
pub struct ParseError {
    msg: String,
}

impl ParseError {
    pub fn new(err: &str) -> ParseError {
        ParseError {
            msg: err.to_owned(),
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ParseError]: {}", self.msg)
    }
}

// automatic due to Display
impl std::error::Error for ParseError {}

// Type of a function value - subset of FnDeclData
// Params: care only about types not names
#[derive(Debug, Clone, PartialEq)]
pub struct FnTypeData {
    pub params: Vec<Type>,
    pub ret_type: Type,
}

impl Display for FnTypeData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Format parameters
        let params_str = if self.params.is_empty() {
            "()".to_string()
        } else {
            let params_display: Vec<String> = self.params.iter().map(|x| x.to_string()).collect();
            format!("({})", params_display.join(", "))
        };

        // Format return type
        let ret_type_str = match &self.ret_type {
            Type::Unit => "".to_string(),
            _ => format!("{}", self.ret_type),
        };

        // Combine formatted parameters and return type
        let display_str = if ret_type_str.is_empty() {
            format!("fn{}", params_str)
        } else {
            format!("fn{} -> {}", params_str, ret_type_str)
        };

        write!(f, "{}", display_str)
    }
}

// Type annotation corresponding to compile time types
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    UserFn(Box<FnTypeData>),
    BuiltInFn, // type checking done separately since it can be polymorphic unlike user fn
    ThreadId,  // result of spawn
    Semaphore,
    Unit,        // void type like Rust
    Unitialised, // Type for variables that exist in a block but not yet declared - only used for TyEnv
}

impl Type {
    // Cast to fn type
    pub fn to_fn_type(&self) -> Option<Box<FnTypeData>> {
        match self {
            Self::UserFn(ty) => Some(ty.to_owned()),
            _ => None,
        }
    }
}

impl Type {
    /// Converts string to primitive type.
    pub fn from_string(input: &str) -> Result<Type, ParseError> {
        match input {
            "int" => Ok(Self::Int),
            "bool" => Ok(Self::Bool),
            "float" => Ok(Self::Float),
            "str" => Ok(Self::String),
            "sem" => Ok(Self::Semaphore),
            _ => Err(ParseError::new(&format!(
                "Unknown primitive type: {}",
                input
            ))),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string: String = match self {
            Self::Int => "int".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Float => "float".to_string(),
            Self::Unit => "()".to_string(),
            Self::Unitialised => "uninit".to_string(),
            Self::BuiltInFn => "builtin_fn".to_string(),
            Self::String => "str".to_string(),
            Self::UserFn(fn_ty) => fn_ty.to_string(),
            Self::ThreadId => "tid".to_string(),
            Self::Semaphore => "sem".to_string(),
        };

        write!(f, "{}", string)
    }
}
