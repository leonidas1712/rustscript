use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BinOp {
    /// Addition of two values of the same type (int or float or string)
    Add,
    /// Subtraction of two values of the same type (int or float)
    Sub,
    /// Multiplication of two values of the same type (int or float)
    Mul,
    /// Division of two values of the same type (int or float)
    Div,
    /// Modulo of two values of the same type (int)
    Mod,
    /// Greater than comparison of two values of the same type (int or float)
    Gt,
    /// Less than comparison of two values of the same type (int or float)
    Lt,
    /// Equality comparison of two values of the same type (bool or int or float or string)
    Eq,
    /// Logical AND of two values of the same type (bool)
    And,
    /// Logical OR of two values of the same type (bool)
    Or,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UnOp {
    /// Negation of a value of the same type (int or float)
    Neg,
    /// Logical negation of a value of the same type (bool)
    Not,
}
