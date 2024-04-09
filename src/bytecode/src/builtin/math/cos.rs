use std::{cell::RefCell, rc::Rc};

use anyhow::Result;

use crate::{Environment, FnType, Value, W};

pub const COS_SYM: &str = "cos";

pub fn cos(global_env: Rc<RefCell<Environment>>) -> Value {
    Value::Closure {
        fn_type: FnType::Builtin,
        sym: COS_SYM.into(),
        prms: vec!["x".into()],
        addr: 0,
        env: W(global_env),
    }
}

pub fn cos_impl(x: &Value) -> Result<Value> {
    let x: f64 = x.clone().try_into()?;
    Ok(Value::Float(x.cos()))
}