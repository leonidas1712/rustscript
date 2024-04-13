use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use anyhow::Result;
use bytecode::{ByteCode, Semaphore, ThreadID};

use crate::{micro_code, Thread, ThreadState, VmError};

const DEFAULT_TIME_QUANTUM: Duration = Duration::from_millis(100);
const MAIN_THREAD_ID: i64 = 1;

/// The runtime of the virtual machine.
/// It contains the instructions to execute, the current thread, and the ready and suspended threads.
/// The instructions are the bytecode instructions to execute.
/// The ready queue is a queue of threads that are ready to run.
/// The suspended queue is a queue of threads that are waiting for some event to occur.
/// The running thread is the thread that is currently executing.
pub struct Runtime {
    time: Instant,
    time_quantum: Duration,
    pub instrs: Vec<ByteCode>,
    pub signal: Option<Semaphore>,
    pub thread_count: i64,
    pub thread_states: HashMap<ThreadID, ThreadState>,
    pub current_thread: Thread,
    pub ready_queue: VecDeque<Thread>,
    pub blocked_queue: VecDeque<Thread>,
    pub zombie_threads: HashMap<ThreadID, Thread>,
}

impl Runtime {
    pub fn new(instrs: Vec<ByteCode>) -> Self {
        let mut thread_states = HashMap::new();
        thread_states.insert(MAIN_THREAD_ID, ThreadState::Ready);
        let current_thread = Thread::new(MAIN_THREAD_ID);

        Runtime {
            time: Instant::now(),
            time_quantum: DEFAULT_TIME_QUANTUM,
            instrs,
            signal: None,
            thread_count: 1,
            thread_states,
            current_thread,
            ready_queue: VecDeque::new(),
            blocked_queue: VecDeque::new(),
            zombie_threads: HashMap::new(),
        }
    }

    pub fn set_time_quantum(&mut self, time_quantum: Duration) {
        self.time_quantum = time_quantum;
    }
}

/// Run the program until it is done.
///
/// # Arguments
///
/// * `rt` - The runtime to run.
///
/// # Returns
///
/// The runtime after the program has finished executing.
///
/// # Errors
///
/// If an error occurs during execution.
pub fn run(mut rt: Runtime) -> Result<Runtime> {
    loop {
        if rt.time_quantum_expired() {
            rt = rt.yield_current_thread();
        }

        if rt.should_yield_current_thread() {
            rt = rt.yield_current_thread();
        }

        if rt.is_current_thread_blocked() {
            rt = rt.block_current_thread();
        }

        if rt.is_post_signaled() {
            rt = rt.signal_post();
        }

        if rt.is_current_thread_joining() {
            rt = rt.join_current_thread()?;
        }

        let instr = rt.fetch_instr()?;

        execute(&mut rt, instr)?;

        if !rt.is_current_thread_done() {
            continue;
        }

        if !rt.is_current_main_thread() {
            rt = rt.zombify_current_thread();
            continue;
        }

        // If the main thread is done, then the program is done.
        break;
    }

    Ok(rt)
}

/// Execute a single instruction, returning whether the program is done.
///
/// # Arguments
///
/// * `rt` - The runtime to execute the instruction on.
///
/// * `instr` - The instruction to execute.
///
/// # Returns
///
/// Whether the program is done executing.
///
/// # Errors
///
/// If an error occurs during execution.
pub fn execute(rt: &mut Runtime, instr: ByteCode) -> Result<()> {
    match instr {
        ByteCode::DONE => micro_code::done(rt)?,
        ByteCode::ASSIGN(sym) => micro_code::assign(rt, sym)?,
        ByteCode::LD(sym) => micro_code::ld(rt, sym)?,
        ByteCode::LDC(val) => micro_code::ldc(rt, val)?,
        ByteCode::LDF(addr, prms) => micro_code::ldf(rt, addr, prms)?,
        ByteCode::POP => micro_code::pop(rt)?,
        ByteCode::UNOP(op) => micro_code::unop(rt, op)?,
        ByteCode::BINOP(op) => micro_code::binop(rt, op)?,
        ByteCode::JOF(pc) => micro_code::jof(rt, pc)?,
        ByteCode::GOTO(pc) => micro_code::goto(rt, pc)?,
        ByteCode::RESET(ft) => micro_code::reset(rt, ft)?,
        ByteCode::ENTERSCOPE(syms) => micro_code::enter_scope(rt, syms)?,
        ByteCode::EXITSCOPE => micro_code::exit_scope(rt)?,
        ByteCode::CALL(arity) => micro_code::call(rt, arity)?,
        ByteCode::SPAWN(addr) => micro_code::spawn(rt, addr)?,
        ByteCode::JOIN(tid) => micro_code::join(rt, tid)?,
        ByteCode::YIELD => micro_code::yield_(rt)?,
        ByteCode::WAIT => micro_code::wait(rt)?,
        ByteCode::POST => micro_code::post(rt)?,
    }
    Ok(())
}

impl Runtime {
    /// Fetch the next instruction to execute.
    /// This will increment the program counter of the current thread.
    ///
    /// # Returns
    ///
    /// The next instruction to execute.
    ///
    /// # Errors
    ///
    /// If the program counter is out of bounds.
    pub fn fetch_instr(&mut self) -> Result<ByteCode> {
        let instr = self
            .instrs
            .get(self.current_thread.pc)
            .cloned()
            .ok_or(VmError::PcOutOfBounds(self.current_thread.pc))?;
        self.current_thread.pc += 1;
        Ok(instr)
    }
}

impl Runtime {
    /// Get the current state of the current thread.
    /// Panics if the current thread is not found.
    pub fn get_current_thread_state(&self) -> ThreadState {
        let current_thread_id = self.current_thread.thread_id;
        self.thread_states
            .get(&current_thread_id)
            .ok_or(VmError::ThreadNotFound(current_thread_id))
            .expect("Current thread not found")
            .clone()
    }

    pub fn set_thread_state(&mut self, thread_id: ThreadID, state: ThreadState) {
        self.thread_states.insert(thread_id, state);
    }

    /// Check if the time quantum has expired.
    /// The time quantum is the maximum amount of time a thread can run before it is preempted.
    pub fn time_quantum_expired(&self) -> bool {
        self.time.elapsed() >= self.time_quantum
    }

    /// Check if the current thread should yield.
    /// This is set by the `YIELD` instruction.
    pub fn should_yield_current_thread(&self) -> bool {
        self.get_current_thread_state() == ThreadState::Yielded
    }

    /// Yield the current thread. Set the state of the current thread to `Ready` and push it onto the ready queue.
    /// Pop the next thread from the ready queue and set it as the current thread.
    /// The timer is reset to the current time.
    /// Panics if the current thread is not found.
    pub fn yield_current_thread(mut self) -> Self {
        let current_thread_id = self.current_thread.thread_id;
        self.set_thread_state(current_thread_id, ThreadState::Ready);

        let current_thread = self.current_thread;
        self.ready_queue.push_back(current_thread);

        let next_ready_thread = self
            .ready_queue
            .pop_front()
            .expect("No threads in ready queue");

        self.current_thread = next_ready_thread;
        self.time = Instant::now(); // Reset the time
        self
    }

    /// Zombify the current thread. Set the state of the current thread to `Zombie` and add it into the zombie threads.
    /// Pop the next thread from the ready queue and set it as the current thread.
    pub fn zombify_current_thread(mut self) -> Self {
        let current_thread = self.current_thread;
        let current_thread_id = current_thread.thread_id;
        let next_ready_thread = self
            .ready_queue
            .pop_front()
            .expect("No threads in ready queue");

        self.zombie_threads
            .insert(current_thread_id, current_thread);
        self.thread_states
            .insert(current_thread_id, ThreadState::Zombie);

        self.current_thread = next_ready_thread;
        self
    }

    pub fn is_current_main_thread(&self) -> bool {
        self.current_thread.thread_id == MAIN_THREAD_ID
    }

    pub fn is_current_thread_done(&self) -> bool {
        self.get_current_thread_state() == ThreadState::Done
    }

    pub fn is_current_thread_joining(&self) -> bool {
        matches!(self.get_current_thread_state(), ThreadState::Joining(_))
    }

    /// Join the current thread with the thread with the given ThreadID based on the current thread's state.
    /// If the thread to join is in zombie state, then the current thread will be set to ready and the result
    /// of the zombie thread will be pushed onto the current thread's operand stack. The zombie thread is deallocated.
    /// If the thread to join is not found, then panic.
    /// Otherwise, the current thread will yield.
    pub fn join_current_thread(mut self) -> Result<Self> {
        let current_thread_id = self.current_thread.thread_id;

        let ThreadState::Joining(tid) = self.get_current_thread_state() else {
            panic!("Current thread is not joining");
        };

        let thread_to_join_state = self.thread_states.get(&tid);

        match thread_to_join_state {
            // If the thread to join does not exist, then panic
            None => {
                panic!("Thread to join not found");
            }
            // If the thread to join is in zombie state, then the current thread will be set to ready
            Some(ThreadState::Zombie) => {
                self.set_thread_state(current_thread_id, ThreadState::Ready);
                let mut zombie_thread = self
                    .zombie_threads
                    .remove(&tid)
                    .ok_or(VmError::ThreadNotFound(tid))?;

                let result = zombie_thread
                    .operand_stack
                    .pop()
                    .ok_or(VmError::OperandStackUnderflow)?;

                self.thread_states.remove(&tid); // Deallocate the zombie thread
                self.current_thread.operand_stack.push(result);
                Ok(self)
            }
            // Otherwise we will yield the current thread
            _ => {
                self.current_thread.pc -= 1; // Decrement the program counter to re-execute the join instruction
                let rt = self.yield_current_thread();
                Ok(rt)
            }
        }
    }
}

impl Runtime {
    pub fn is_current_thread_blocked(&self) -> bool {
        matches!(self.get_current_thread_state(), ThreadState::Blocked(_))
    }

    pub fn block_current_thread(mut self) -> Self {
        let current_thread = self.current_thread;
        self.blocked_queue.push_back(current_thread);

        let next_ready_thread = self
            .ready_queue
            .pop_front()
            .expect("No threads in ready queue");

        self.current_thread = next_ready_thread;
        self
    }

    pub fn is_post_signaled(&self) -> bool {
        self.signal.is_some()
    }

    pub fn signal_post(mut self) -> Self {
        let sem = self.signal.take().expect("No semaphore to signal");

        {
            let mut sem_guard = sem.lock().unwrap();
            *sem_guard += 1;
        }

        let mut blocked_threads = VecDeque::new();

        for thread in self.blocked_queue.drain(..) {
            let ThreadState::Blocked(sem_other) = self
                .thread_states
                .get(&thread.thread_id)
                .expect("Thread not found")
                .clone()
            else {
                continue;
            };

            if sem == sem_other {
                self.thread_states
                    .insert(thread.thread_id, ThreadState::Ready);
                self.ready_queue.push_back(thread);
            } else {
                blocked_threads.push_back(thread);
            }
        }

        self.blocked_queue = blocked_threads;
        self
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use anyhow::{Ok, Result};
    use bytecode::{builtin, BinOp, ByteCode, FrameType, Symbol, UnOp, Value};

    #[test]
    fn test_pc() {
        let instrs = vec![
            ByteCode::ldc(42),
            ByteCode::POP,
            ByteCode::ldc(42),
            ByteCode::POP,
            ByteCode::DONE,
        ];
        let rt = Runtime::new(instrs);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.pc, 5);

        let rt = Runtime::new(vec![
            ByteCode::ldc(false),
            ByteCode::JOF(3),
            ByteCode::POP, // This will panic since there is no value on the stack
            ByteCode::DONE,
        ]);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.pc, 4);

        let rt = Runtime::new(vec![
            ByteCode::ldc(true),
            ByteCode::JOF(3), // jump to pop instruction
            ByteCode::DONE,
            ByteCode::POP, // This will panic since there is no value on the stack
            ByteCode::DONE,
        ]);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.pc, 3);

        let rt = Runtime::new(vec![
            ByteCode::GOTO(2),
            ByteCode::POP, // This will panic since there is no value on the stack
            ByteCode::DONE,
        ]);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.pc, 3);
    }

    #[test]
    fn test_arithmetic() {
        // 42 + 42
        let instrs = vec![
            ByteCode::ldc(42),
            ByteCode::ldc(42),
            ByteCode::BINOP(BinOp::Add),
            ByteCode::DONE,
        ];
        let rt = Runtime::new(instrs);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.operand_stack, vec![Value::Int(84)]);

        // -(42 - 123)
        let instrs = vec![
            ByteCode::ldc(42),
            ByteCode::ldc(123),
            ByteCode::BINOP(BinOp::Sub),
            ByteCode::UNOP(UnOp::Neg),
            ByteCode::DONE,
        ];
        let rt = Runtime::new(instrs);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.operand_stack, vec![Value::Int(81)]);

        // (2 * 3) > 9
        let instrs = vec![
            ByteCode::ldc(2),
            ByteCode::ldc(3),
            ByteCode::BINOP(BinOp::Mul),
            ByteCode::ldc(9),
            ByteCode::BINOP(BinOp::Gt),
            ByteCode::DONE,
        ];
        let rt = Runtime::new(instrs);
        let rt = run(rt).unwrap();
        assert_eq!(rt.current_thread.operand_stack, vec![Value::Bool(false)]);
    }

    #[test]
    fn test_assignment() {
        let instrs = vec![
            ByteCode::ldc(42),
            ByteCode::assign("x"),
            ByteCode::ldc(43),
            ByteCode::assign("y"),
            ByteCode::ldc(44),
            ByteCode::assign("x"),
            ByteCode::DONE,
        ];

        let rt = Runtime::new(instrs);
        rt.current_thread
            .env
            .borrow_mut()
            .set("x", Value::Unitialized);
        rt.current_thread
            .env
            .borrow_mut()
            .set("y", Value::Unitialized);

        let rt = run(rt).unwrap();
        assert_eq!(
            rt.current_thread.env.borrow().get(&"x".to_string()),
            Some(Value::Int(44))
        );
        assert_eq!(
            rt.current_thread.env.borrow().get(&"y".to_string()),
            Some(Value::Int(43))
        );
    }

    #[test]
    fn test_fn_call() -> Result<()> {
        // fn simple(n) {
        //     return n;
        // }
        // simple(42)
        let instrs = vec![
            ByteCode::enterscope(vec!["simple"]),
            ByteCode::ldf(3, vec!["n"]),
            ByteCode::GOTO(5), // Jump to the end of the function
            // Body of simple
            ByteCode::ld("n"), // Load the value of n onto the stacks
            ByteCode::RESET(FrameType::CallFrame), // Return from the function
            ByteCode::assign("simple"), // Assign the function to the symbol
            ByteCode::ld("simple"), // Load the function onto the stack
            ByteCode::ldc(42), // Load the argument onto the stack
            ByteCode::CALL(1), // Call the function with 1 argument
            ByteCode::EXITSCOPE,
            ByteCode::DONE,
        ];

        let rt = Runtime::new(instrs);
        let mut rt = run(rt)?;

        let result = rt.current_thread.operand_stack.pop().unwrap();
        assert_eq!(result, Value::Int(42));
        assert_eq!(rt.current_thread.runtime_stack.len(), 0);

        Ok(())
    }

    #[test]
    fn test_global_constants() -> Result<()> {
        let instrs = vec![ByteCode::ld(builtin::PI_SYM), ByteCode::DONE];

        let rt = Runtime::new(instrs);
        let rt = run(rt)?;
        assert_eq!(
            rt.current_thread.operand_stack,
            vec![Value::Float(std::f64::consts::PI)]
        );

        let instrs = vec![ByteCode::ld(builtin::MAX_INT_SYM), ByteCode::DONE];

        let rt = Runtime::new(instrs);
        let rt = run(rt)?;

        assert_eq!(
            rt.current_thread.operand_stack,
            vec![Value::Int(std::i64::MAX)]
        );

        Ok(())
    }

    #[test]
    fn test_global_functions() -> Result<()> {
        let instrs = vec![
            ByteCode::ld(builtin::STRING_LEN_SYM),
            ByteCode::ldc("Hello, world!"),
            ByteCode::CALL(1),
            ByteCode::DONE,
        ];

        let rt = Runtime::new(instrs);
        let rt = run(rt)?;

        assert_eq!(rt.current_thread.operand_stack, vec![Value::Int(13)]);

        let instrs = vec![
            ByteCode::ld(builtin::ABS_SYM),
            ByteCode::ldc(-42),
            ByteCode::CALL(1),
            ByteCode::DONE,
        ];

        let rt = Runtime::new(instrs);
        let rt = run(rt)?;

        assert_eq!(rt.current_thread.operand_stack, vec![Value::Int(42)]);

        Ok(())
    }

    #[test]
    fn test_concurrency_01() -> Result<()> {
        let instrs = vec![ByteCode::SPAWN(1), ByteCode::DONE];

        let mut rt = Runtime::new(instrs);
        rt.set_time_quantum(Duration::from_millis(u64::MAX)); // Set the time quantum to infinity
        let rt = run(rt)?;

        // There is one thread in the ready queue
        assert_eq!(rt.ready_queue.len(), 1);
        // The spawned instruction pushes 0 onto the operand stack of the child thread
        assert_eq!(rt.ready_queue[0].operand_stack, vec![Value::Int(0)]);
        // The spawn instruction pushes the child thread ID onto the parent thread's operand stack
        assert_eq!(
            rt.current_thread.operand_stack,
            vec![Value::Int(MAIN_THREAD_ID + 1)]
        );

        Ok(())
    }

    #[test]
    fn test_concurrency_02() -> Result<()> {
        // fn simple(n) {
        //    return n;
        // }
        //
        // spawn simple(123);
        // join 2
        let instrs = vec![
            ByteCode::enterscope(vec!["simple"]),
            ByteCode::ldf(3, vec!["n"]),
            ByteCode::GOTO(5), // Jump past function body
            ByteCode::ld("n"),
            ByteCode::RESET(FrameType::CallFrame),
            ByteCode::assign("simple"),
            ByteCode::SPAWN(8), // Parent operand stack will have child tid 2, child operand stack will have
            ByteCode::GOTO(13), // Parent jump past CALL and DONE
            ByteCode::POP,
            ByteCode::ld("simple"),
            ByteCode::ldc(123),
            ByteCode::CALL(1),
            ByteCode::DONE,
            ByteCode::JOIN(MAIN_THREAD_ID + 1), // Parent thread joins the child thread
            ByteCode::DONE,
        ];

        let rt = Runtime::new(instrs);
        let mut rt = run(rt)?;

        println!("{:?}", rt.current_thread.operand_stack);

        assert_eq!(
            rt.current_thread.operand_stack.pop().unwrap(),
            Value::Int(123)
        );

        Ok(())
    }

    #[test]
    fn test_concurrency_03() -> Result<()> {
        // let count = 0;
        // fn infinite_increment() {
        //    loop {
        //        count = count + 1;
        //    }
        // }
        // spawn infinite_increment();
        // yield;
        // // no join

        let empty_str_arr: Vec<Symbol> = vec![];

        let instrs = vec![
            ByteCode::enterscope(vec!["count", "infinite_increment"]),
            ByteCode::ldc(0),
            ByteCode::assign("count"), // Set count to 0
            ByteCode::ldf(6, empty_str_arr),
            ByteCode::assign("infinite_increment"), // assign function
            ByteCode::GOTO(11),                     // Jump past function body
            ByteCode::ld("count"),                  // Start of function body
            ByteCode::ldc(1),
            ByteCode::BINOP(BinOp::Add),
            ByteCode::assign("count"),
            ByteCode::GOTO(6),   // End of function body
            ByteCode::SPAWN(13), // Parent operand stack will have child tid 2, child operand stack will have
            ByteCode::GOTO(17),  // Parent jump past CALL and DONE
            ByteCode::POP,
            ByteCode::ld("infinite_increment"),
            ByteCode::CALL(0),
            ByteCode::DONE,
            ByteCode::YIELD, // Parent thread yields to child thread
            ByteCode::DONE,
        ];

        let mut rt = Runtime::new(instrs);
        rt.set_time_quantum(Duration::from_millis(1000)); // Set the time quantum to 1 second
        let rt = run(rt)?;

        let final_count: i64 = rt
            .current_thread
            .env
            .borrow()
            .get(&"count".to_string())
            .expect("Count not in environment")
            .try_into()?;

        assert!(final_count > 0);
        Ok(())
    }
}
