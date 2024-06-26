use std::{cell::RefCell, collections::HashMap, rc::Weak};

use bytecode::{weak_clone, EnvWeak, Environment, StackFrame, Value, W};

use crate::{Runtime, Thread};

/// Runtime methods at runtime.
impl Runtime {
    /// Mark and sweep the environment registry.
    /// This will remove all environments that are no longer referenced.
    ///
    /// - Mark environment x -> env_registry.get(x) = true
    /// - Sweep environment x -> env_registry.remove(x) if env_registry.get(x) = false
    /// - Clean up -> reset env_registry.get(x) = false
    ///
    /// Traverse through all the threads, for each thread:
    ///   - Mark its current environment and the environment of closure values in the current environment,
    ///     and the chain of parent environments.
    ///   - Go through the runtime stack and mark all the environments and environment of closure values in
    ///     their respective environment, and the chain of parent environments
    ///   - Go through the operand stack and mark all the environments of closure values, and the chain of parent environments
    #[inline]
    pub fn mark_and_weep(self) -> Self {
        let marked = mark(&self);
        sweep(self, marked)
    }
}

fn mark(rt: &Runtime) -> HashMap<EnvWeak, bool> {
    if rt.debug {
        println!("Mark begin")
    }

    let mut marked = env_hashmap(rt);

    // Mark the current thread
    marked = mark_thread(marked, &rt.current_thread);

    // Mark the ready queue
    for thread in rt.ready_queue.iter() {
        marked = mark_thread(marked, thread);
    }

    // Mark the blocked queue
    for (thread, _) in rt.blocked_queue.iter() {
        marked = mark_thread(marked, thread);
    }

    // Zombie threads will be ignored

    marked
}

fn sweep(mut rt: Runtime, m: HashMap<EnvWeak, bool>) -> Runtime {
    if rt.debug {
        println!("Sweep begin")
    }

    let registry = rt
        .env_registry
        .drain()
        .filter(|env| *m.get(&W(weak_clone(env))).unwrap_or(&false))
        .collect();
    rt.env_registry = registry;

    if rt.debug {
        println!(
            "Sweep end, {} environments removed",
            m.len() - rt.env_registry.len()
        )
    }

    rt // Any environment that is not marked will be removed from the registry and dropped
}

fn env_hashmap(rt: &Runtime) -> HashMap<EnvWeak, bool> {
    let mut m = HashMap::new();
    for env in rt.env_registry.iter() {
        m.insert(W(weak_clone(env)), false);
    }
    m
}

fn mark_thread(mut m: HashMap<EnvWeak, bool>, t: &Thread) -> HashMap<EnvWeak, bool> {
    m = mark_env(m, &t.env);
    m = mark_operand_stack(m, &t.operand_stack);
    m = mark_runtime_stack(m, &t.runtime_stack);
    m
}

fn mark_env(
    mut m: HashMap<EnvWeak, bool>,
    env: &Weak<RefCell<Environment>>,
) -> HashMap<EnvWeak, bool> {
    let is_marked = m
        .get_mut(&W(env.clone()))
        .expect("Environment must be in the registry");

    match is_marked {
        true => return m, // Already marked
        false => *is_marked = true,
    }

    let env = env
        .upgrade()
        .expect("Environment must still be referenced to be marked");

    if let Some(parent) = &env.borrow().parent {
        m = mark_env(m, parent);
    }

    m
}

fn mark_operand_stack(mut m: HashMap<EnvWeak, bool>, os: &[Value]) -> HashMap<EnvWeak, bool> {
    for val in os.iter() {
        if let Value::Closure { env, .. } = val {
            m = mark_env(m, env);
        }
    }
    m
}

fn mark_runtime_stack(mut m: HashMap<EnvWeak, bool>, rs: &[StackFrame]) -> HashMap<EnvWeak, bool> {
    for frame in rs.iter() {
        m = mark_env(m, &frame.env);
    }
    m
}

#[cfg(test)]
mod tests {
    use crate::run;

    use super::*;

    use anyhow::Result;
    use bytecode::*;

    #[test]
    fn test_gc_01() -> Result<()> {
        // {
        //   fn garbage() {}
        // }
        // // garbage is out of scope and not reachable, so the environment should be removed
        let empty_vec: Vec<Symbol> = vec![];

        let instrs = vec![
            ByteCode::enterscope(empty_vec.clone()), // Program scope
            ByteCode::enterscope(vec!["garbage"]),   // Block scope
            ByteCode::ldf(0, empty_vec.clone()),
            ByteCode::assign("garbage"),
            ByteCode::EXITSCOPE,
            ByteCode::EXITSCOPE,
            ByteCode::DONE,
        ];

        let mut rt = Runtime::new(instrs);
        rt.set_debug_mode();
        let rt = run(rt)?;
        assert_eq!(rt.env_registry.len(), 3); // Global env, program env, block env

        let rt = rt.mark_and_weep();
        assert_eq!(rt.env_registry.len(), 1); // Only the global environment should be left

        Ok(())
    }

    #[test]
    fn test_gc_02() -> Result<()> {
        // fn higher_order(x) {
        //   return y => x + y;
        // }
        //
        // const add10 = higher_order(10);
        //
        // const result = add10(20);
        //
        // println(result); // 30

        let instrs = vec![
            // PC: 0
            ByteCode::enterscope(vec!["higher_order", "add10", "result"]), // Program scope
            // PC: 1
            ByteCode::ldf(4, vec!["x"]), // higher_order
            // PC: 2
            ByteCode::assign("higher_order"),
            // PC: 3
            ByteCode::GOTO(11), // Jump past higher_order body
            // PC: 4
            ByteCode::ldf(6, vec!["y"]), // higher_order annonymous function
            // PC: 5
            ByteCode::GOTO(10), // Jump past annonymous function body
            // PC: 6
            ByteCode::ld("x"),
            // PC: 7
            ByteCode::ld("y"),
            // PC: 8
            ByteCode::BINOP(BinOp::Add),
            // PC: 9
            ByteCode::RESET(FrameType::CallFrame), // reset instruction for annonymous function
            // PC: 10
            ByteCode::RESET(FrameType::CallFrame), // reset instruction for higher_order
            // PC: 11
            ByteCode::ld("higher_order"),
            // PC: 12
            ByteCode::ldc(10),
            // PC: 13
            ByteCode::CALL(1),
            // PC: 14
            ByteCode::assign("add10"),
            // PC: 15
            ByteCode::ld("add10"),
            // PC: 16
            ByteCode::ldc(20),
            // PC: 17
            ByteCode::CALL(1),
            // PC: 18
            ByteCode::assign("result"),
            // PC: 19
            ByteCode::ld("println"),
            // PC: 20
            ByteCode::ld("result"),
            // PC: 21
            ByteCode::CALL(1),
            // PC: 22
            ByteCode::EXITSCOPE,
            // PC: 23
            ByteCode::DONE,
        ];

        let mut rt = Runtime::new(instrs);
        rt.set_debug_mode();
        let rt = run(rt)?;

        let rt = rt.mark_and_weep();
        assert_eq!(rt.env_registry.len(), 1); // Only the global environment should be left

        Ok(())
    }
}
