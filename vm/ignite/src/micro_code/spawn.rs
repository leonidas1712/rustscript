use anyhow::Result;

use crate::Runtime;

/// Spawn a new thread that clones the main thread at the time of the spawn.
/// The new thread is added to the thread state hashmap with a state of Ready.
/// The new thread is given a unique thread ID.
/// The new thread is added to the ready queue.
/// This thread ID is pushed onto the operand stack of the parent thread.
/// 0 is pushed onto the operand stack of the child thread.
///
/// # Arguments
///
/// * `rt` - The runtime to spawn a new thread in.
///
/// # Errors
///
/// Infallible.
pub fn spawn(mut rt: Runtime, addr: usize) -> Result<Runtime> {
    rt.thread_count += 1;

    let child_thread_id = rt.thread_count;
    let mut child_thread = rt.current_thread.spawn_child(child_thread_id, addr);

    // 0 is pushed onto the operand stack of the child thread.
    child_thread.operand_stack.push(0.into());
    // The child thread ID is pushed onto the operand stack of the parent thread.
    rt.current_thread.operand_stack.push(child_thread_id.into());

    rt.ready_queue.push_back(child_thread);
    Ok(rt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn() -> Result<()> {
        let rt = Runtime::new(vec![]);
        let rt = spawn(rt, 0)?;
        assert_eq!(rt.thread_count, 2);
        assert_eq!(rt.ready_queue.len(), 1);
        Ok(())
    }
}
