use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use crate::W;

pub type Semaphore = W<Arc<Mutex<u64>>>;

impl PartialEq for Semaphore {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Clone for Semaphore {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Debug for Semaphore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Semaphore({})", self.lock().unwrap())
    }
}
