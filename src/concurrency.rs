//! Concurrency infrastructure for the work queue and thread pools.

use scc::HashSet as ConcurrentHashSet;
use std::{
    collections::BinaryHeap,
    sync::{Condvar, Mutex},
};

/// A piece of data with associated cost.
///
/// `WithCost` objects are ordered according to their cost, so a <= b iff a.cost
/// >= b.cost.
#[derive(PartialEq, Eq)]
pub struct WithCost<T>(pub T, pub isize);

impl<T: Eq + Ord> Ord for WithCost<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (-self.1).cmp(&(-other.1)).then(self.0.cmp(&other.0))
    }
}

impl<T: Eq + Ord> PartialOrd for WithCost<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Inner structure of the worklist
struct Inner<T> {
    heap: BinaryHeap<WithCost<T>>,
    done: bool,
    /// number of threads waiting on the worklist.  if this list reaches
    /// workers, then we're headed for a deadlock because there are no more
    /// items to process, so all threads will receive a Done message.
    waiting: usize,
    /// number of worker threads, fixed when the worklist is created
    workers: usize,
}

/// Abstracted worklist to allow different search strategies/worklist structures
pub struct Worklist<T> {
    inner: Mutex<Inner<T>>,
    // Sets of values that are already processed, indexed by cost (cost as usize).
    // We have it here so that we can avoid locking the worklist repeatedly for this check.
    pub seen: Vec<ConcurrentHashSet<T>>,
    cv: Condvar,
    /// If true, callers promise never to push cost < N after calling
    /// retain_for_max_cost(N, ...). Enables last_cleaned advancement.
    is_extensive: bool,
    /// How far we've cleaned seen sets (exclusive upper bound on cost index).
    /// Only meaningful when Worklist::is_extensive is true.
    ///
    /// This is behind a mutex so that retain_for_max_cost can be treated as a
    /// critical region.
    last_cleaned: Mutex<usize>,
}

impl<T: Eq + Ord + std::hash::Hash + Clone> Worklist<T> {
    pub fn new<const N: usize>(
        xs: [WithCost<T>; N],
        workers: usize,
        max_cost: usize,
        is_extensive: bool,
    ) -> Worklist<T> {
        let heap = BinaryHeap::from(xs);
        let seen = (0..=max_cost).map(|_| ConcurrentHashSet::new()).collect();
        Worklist {
            inner: Mutex::new(Inner {
                heap,
                workers,
                waiting: 0,
                done: false,
            }),
            cv: Condvar::new(),
            seen,
            is_extensive,
            last_cleaned: Mutex::new(0),
        }
    }

    /// Remove entries from `seen[last_clened..cost]` that do not satisfy
    /// `predicate`.
    ///
    /// Does nothing if `is_extensive` is false.
    pub fn retain_up_to_max_cost<F>(&self, max_cost: usize, predicate: F)
    where
        F: Fn(&T) -> bool,
    {
        if !self.is_extensive {
            return;
        }
        let mut last_cleaned = self.last_cleaned.lock().unwrap();
        for s in *last_cleaned..max_cost {
            self.seen[s].retain_sync(|item| predicate(item));
        }
        *last_cleaned = last_cleaned.max(max_cost);
    }

    pub fn push(&self, g: WithCost<T>) {
        let mut inner = self.inner.lock().unwrap();
        log::debug!(
            "  [push] waiting: {}\t|queue|: {}",
            inner.waiting,
            inner.heap.len()
        );
        if inner.done {
            self.cv.notify_all();
        } else {
            inner.heap.push(g);
            self.cv.notify_all();
        }
    }

    pub fn push_all(&self, gs: Vec<WithCost<T>>) {
        let mut inner = self.inner.lock().unwrap();
        log::debug!(
            "  [push_all] waiting: {}\t|queue|: {}",
            inner.waiting,
            inner.heap.len()
        );
        if inner.done {
            self.cv.notify_all();
        } else {
            inner.heap.extend(gs);
            self.cv.notify_all();
        }
    }

    pub fn pop(&self) -> Task<WithCost<T>> {
        let mut inner = self.inner.lock().unwrap();

        loop {
            if inner.done {
                return Task::Done;
            }
            log::debug!(
                "  [pop]  waiting: {}\t|queue|: {}",
                inner.waiting,
                inner.heap.len()
            );
            if let Some(task) = inner.heap.pop() {
                let cost_idx = task.1 as usize;
                if self.seen[cost_idx].insert_sync(task.0.clone()).is_ok() {
                    log::debug!("  dispatching a task");
                    return Task::Todo(task);
                }
            } else {
                log::debug!("  putting one thread to sleep");
                inner.waiting += 1;
                if inner.waiting == inner.workers {
                    // all threads are waiting, so we're done
                    inner.done = true;
                    self.cv.notify_all();
                    return Task::Done;
                }
                // wait until awoken by another thread
                inner = self.cv.wait(inner).unwrap();
                inner.waiting = inner.waiting.checked_sub(1).unwrap();
                log::debug!(
                    "  [wakeup] waiting: {}\t|queue|: {}",
                    inner.waiting,
                    inner.heap.len()
                );
            }
        }
    }

    pub fn done(&self) {
        self.inner.lock().unwrap().done = true;
        self.cv.notify_all();
    }

    pub fn is_done(&self) -> bool {
        self.inner.lock().unwrap().done
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().heap.is_empty()
    }
}

/// A task/message to be sent to a worker thread
pub enum Task<T> {
    /// Another thread has reached the terminal condition.
    Done,
    /// A task to process.
    Todo(T),
}
