
#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use anyhow::{bail, Result};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::task::spawn;
use tokio::time::sleep;
use std::fmt::Display;
use std::sync::Arc;


struct Payload<T> {
    idx: usize,
    payload: T,
}
impl<T> From<(usize, T)> for Payload<T> {
    fn from((idx, value): (usize, T)) -> Self {
        Self { idx, payload: value }
    }
}

impl<T> Eq for Payload<T> {}

impl<T> PartialEq<Self> for Payload<T> {
    fn eq(&self, other: &Self) -> bool {
        self.idx.eq(&other.idx)
    }
}

impl<T> PartialOrd<Self> for Payload<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.idx.partial_cmp(&other.idx)
    }
}

impl<T> Ord for Payload<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.idx.cmp(&other.idx)
    }
}

pub type ProducerFn<Payload> = Arc<dyn Fn(usize) -> (usize, Payload) + Send + Sync>;
pub type ConsumerFn<Payload> = dyn Fn(Payload) + Send + Sync;

struct Idx {
    iterations: usize,
    start: usize,
    current: usize,
    next: usize,
}

pub struct ParallelRunner<'b, T> {
    max: usize,
    idx: Arc<RwLock<Idx>>,
    producer: ProducerFn<T>,
    consumer: &'b ConsumerFn<T>,
}
impl<T: Display + Send + Sync + 'static> ParallelRunner<'static, T> {
    pub fn new(max: usize, start_idx: usize, iterations: Option<usize>, producer: ProducerFn<T>, consumer: &'static ConsumerFn<T>) -> Result<Self> {
        if max < 1 {
            bail!("max is null, this isn't allowed");
        }
        Ok(Self {
            max,
            idx: Arc::new(RwLock::new(Idx {
                iterations: iterations.unwrap_or(usize::MAX),
                start: start_idx,
                current: usize::MAX - start_idx,
                next: usize::MAX - start_idx,
            })),
            producer,
            consumer,
        })
    }
    pub async fn run(&mut self) -> Result<()> {
        let mut set: JoinSet<(usize, T)> = JoinSet::new();

        // init join set
        for _ in 0..self.max {
            let n = self.idx.read().await.next;
            self.idx.write().await.next -= 1;
            let p = self.producer.clone();
            set.spawn(async move {
                let Payload{idx, payload} = p(usize::MAX - n).into();
                (usize::MAX - idx, payload)
            });
        }

        let bin_heap = Arc::new(RwLock::new(BinaryHeap::<Payload<T>>::new()));
        let heap = bin_heap.clone();
        let max = self.max;
        let idx = self.idx.clone();
        let producer = self.producer.clone();
        let jh_heap = spawn(async move {
            while let Some(result) = set.join_next().await {
                heap.write().await.push(result?.into());
                if idx.read().await.iterations == 0 {
                    break;
                }
                while set.len() < max {
                    let n = idx.read().await.next;
                    idx.write().await.next -= 1;
                    let p = producer.clone();
                    set.spawn(async move {
                        log::debug!("next: {}", n);
                        let Payload{idx, payload} = p(usize::MAX - n).into();
                        (usize::MAX - idx, payload)
                    });
                }
            }
            anyhow::Ok(())
        });

        let consumer = self.consumer;
        let heap = bin_heap.clone();
        let idx = self.idx.clone();
        let jh_consume = spawn(async move {
            loop {
                if idx.read().await.iterations == 0 {
                    break;
                }
                {
                    let peek = heap.read().await.peek().map(|data| data.idx);
                    if peek.is_none() {
                        sleep(std::time::Duration::from_millis(5)).await;
                        continue;
                    }
                    let cur = idx.read().await.current;
                    if peek.unwrap().lt(&cur) {
                        sleep(std::time::Duration::from_millis(5)).await;
                        continue;
                    }
                }
                if let Some(data) = heap.write().await.pop() {
                    log::debug!("output {:?}", data.idx);
                    consumer(data.payload);
                    idx.write().await.current -= 1;
                    idx.write().await.iterations -= 1;
                }
            }
        });

        let _ = jh_heap.await?;
        jh_consume.await?;

        Ok(())
    }
}

#[cfg(test)]
// currently not used
mod test {
    use anyhow::Result;
    use crate::Container;

    #[tokio::test]
    async fn test() -> Result<()>{
        let mut runner = Container::new(
            5,
            &|| { std::thread::sleep(std::time::Duration::from_secs(1)); 4 },
            &|item| { log::info!("output: {}", item); }
        )?;
        runner.run().await?;
        Ok(())
    }
}


