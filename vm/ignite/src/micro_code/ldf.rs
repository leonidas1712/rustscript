use anyhow::Result;
use bytecode::{FnType, Symbol, Value, W};

use crate::Runtime;

/// Load a closure object onto the operand stack.
///
/// # Arguments
///
/// * `rt` - The runtime to load the closure onto.
///
/// * `addr` - The address of the closure.
///
/// * `prms` - The parameters of the closure.
///
/// # Errors
///
/// Infallible.
#[inline]
pub fn ldf(mut rt: Runtime, addr: usize, prms: Vec<Symbol>) -> Result<Runtime> {
    let closure = Value::Closure {
        fn_type: FnType::User,
        sym: "Closure".to_string(),
        prms,
        addr,
        env: W(rt.current_thread.env.clone()),
    };

    rt.current_thread.operand_stack.push(closure);
    Ok(rt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldf() {
        let mut rt = Runtime::new(vec![]);
        rt = ldf(rt, 0, vec!["x".to_string()]).unwrap();

        let closure = rt.current_thread.operand_stack.pop().unwrap();
        assert_ne!(
            &closure,
            &Value::Closure {
                fn_type: FnType::User,
                sym: "Closure".to_string(),
                prms: vec!["y".to_string()],
                addr: 0,
                env: W(rt.current_thread.env.clone()),
            }
        )
    }
}
