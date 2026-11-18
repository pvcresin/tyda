//! A parallel map whose results are consumed on the calling thread in input order.
//!
//! Batch scans merge per-file results in file order. Running each fixed chunk with
//! `par_iter` and merging after it leaves workers idle at every chunk's tail and during the
//! merge. Here workers keep taking the next item while the caller merges, and at most
//! `window` items are in flight or waiting, so memory stays bounded as with chunks.

use std::collections::VecDeque;
use std::sync::mpsc;

/// Sends `None` for its index when the job ends without a result (a panic), so the caller
/// stops waiting and the scope re-raises the panic.
struct SendOnDrop<'a, R> {
    tx: mpsc::Sender<(usize, Option<R>)>,
    index: usize,
    sent: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<R> Drop for SendOnDrop<'_, R> {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self.tx.send((self.index, None));
        }
    }
}

/// Runs `map` over `items` on `pool` and passes each result to `consume` in input order.
/// `should_stop` is checked before each new item starts; once it returns true, no more
/// items start, and the items already started are still consumed.
pub fn for_each_ordered<T, R, M, C, S>(
    pool: &rayon::ThreadPool,
    items: &[T],
    window: usize,
    map: M,
    mut consume: C,
    mut should_stop: S,
) where
    T: Sync,
    R: Send,
    M: Fn(&T) -> R + Sync,
    C: FnMut(R),
    S: FnMut() -> bool,
{
    if items.is_empty() {
        return;
    }
    let window = window.max(1);
    let (tx, rx) = mpsc::channel::<(usize, Option<R>)>();
    pool.in_place_scope(|scope| {
        let map = &map;
        // `ready[i]` holds the result of item `consumed + i` once it arrives.
        let mut ready: VecDeque<Option<R>> = VecDeque::with_capacity(window);
        let mut started = 0usize;
        let mut consumed = 0usize;
        let mut stopped = false;
        while consumed < started || (!stopped && started < items.len()) {
            while !stopped && started < items.len() && started < consumed + window {
                if should_stop() {
                    stopped = true;
                    break;
                }
                let item = &items[started];
                let mut guard = SendOnDrop {
                    tx: tx.clone(),
                    index: started,
                    sent: false,
                    _marker: std::marker::PhantomData,
                };
                scope.spawn(move |_| {
                    let result = map(item);
                    guard.sent = true;
                    let _ = guard.tx.send((guard.index, Some(result)));
                });
                started += 1;
            }
            if consumed == started {
                break;
            }
            let Ok((index, result)) = rx.recv() else {
                break;
            };
            let Some(result) = result else {
                // A job panicked; the scope re-raises it once the running jobs finish.
                return;
            };
            let slot = index - consumed;
            if ready.len() <= slot {
                ready.resize_with(slot + 1, || None);
            }
            ready[slot] = Some(result);
            while matches!(ready.front(), Some(Some(_))) {
                let result = ready
                    .pop_front()
                    .flatten()
                    .expect("front result is present");
                consumed += 1;
                consume(result);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::for_each_ordered;

    fn pool(threads: usize) -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool")
    }

    #[test]
    fn consumes_in_input_order() {
        let items: Vec<u64> = (0..500).collect();
        let mut seen = Vec::new();
        for_each_ordered(
            &pool(4),
            &items,
            7,
            |&n| {
                // Uneven work so results arrive out of order.
                std::thread::sleep(std::time::Duration::from_micros((n * 37) % 200));
                n * 2
            },
            |value| seen.push(value),
            || false,
        );
        assert_eq!(seen, items.iter().map(|n| n * 2).collect::<Vec<_>>());
    }

    #[test]
    fn stops_starting_items_once_asked() {
        let items: Vec<u64> = (0..100).collect();
        let mut seen = Vec::new();
        let mut checks = 0;
        for_each_ordered(
            &pool(2),
            &items,
            4,
            |&n| n,
            |value| seen.push(value),
            || {
                checks += 1;
                checks > 10
            },
        );
        assert!(seen.len() < items.len());
        assert_eq!(seen, (0..seen.len() as u64).collect::<Vec<_>>());
    }

    #[test]
    fn empty_input_does_nothing() {
        let items: Vec<u64> = Vec::new();
        let mut calls = 0;
        for_each_ordered(&pool(2), &items, 4, |&n| n, |_| calls += 1, || false);
        assert_eq!(calls, 0);
    }

    #[test]
    #[should_panic(expected = "boom")]
    fn job_panic_propagates() {
        let items: Vec<u64> = (0..20).collect();
        for_each_ordered(
            &pool(2),
            &items,
            4,
            |&n| {
                if n == 5 {
                    panic!("boom");
                }
                n
            },
            |_| {},
            || false,
        );
    }
}
