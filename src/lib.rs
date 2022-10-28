
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


struct Data<T> {
    idx: usize,
    value: T,
}
impl<T> From<(usize, T)> for Data<T> {
    fn from((idx, value): (usize, T)) -> Self {
        Self { idx, value }
    }
}

impl<T> Eq for Data<T> {}

impl<T> PartialEq<Self> for Data<T> {
    fn eq(&self, other: &Self) -> bool {
        self.idx.eq(&other.idx)
    }
}

impl<T> PartialOrd<Self> for Data<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.idx.partial_cmp(&other.idx)
    }
}

impl<T> Ord for Data<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.idx.cmp(&other.idx)
    }
}

pub type ProducerFn<T> = Arc<dyn Fn() -> T + Send + Sync>;
pub type ConsumerFn<T> = dyn Fn(T) + Send + Sync;

pub struct ParallelRunner<'b, T> {
    max: usize,
    iterations: Arc<RwLock<usize>>,
    current: Arc<RwLock<usize>>,
    next: Arc<RwLock<usize>>,
    producer: ProducerFn<T>,
    consumer: &'b ConsumerFn<T>,
}
impl<T: Display + Send + Sync + 'static> ParallelRunner<'static, T> {
    pub fn new(max: usize, iterations: Option<usize>, producer: ProducerFn<T>, consumer: &'static ConsumerFn<T>) -> Result<Self> {
        if max < 1 {
            bail!("max is null, this isn't allowed");
        }
        Ok(Self {
            max,
            iterations: Arc::new(RwLock::new(iterations.unwrap_or(usize::MAX))),
            current: Arc::new(RwLock::new(usize::MAX)),
            next: Arc::new(RwLock::new(usize::MAX)),
            producer,
            consumer,
        })
    }
    pub async fn run(&mut self) -> Result<()> {
        let mut set: JoinSet<(usize, T)> = JoinSet::new();

        // init join set
        for _ in 0..self.max {
            let n = *self.next.read().await;
            *self.next.write().await -= 1;
            let p = self.producer.clone();
            set.spawn(async move {
                (n, p())
            });
        }

        let bin_heap = Arc::new(RwLock::new(BinaryHeap::<Data<T>>::new()));
        let heap = bin_heap.clone();
        let max = self.max;
        let next = self.next.clone();
        let iterations = self.iterations.clone();
        let producer = self.producer.clone();
        let jh_heap = spawn(async move {
            while let Some(result) = set.join_next().await {
                heap.write().await.push(result?.into());
                if *iterations.read().await == 0 {
                    break;
                }
                while set.len() < max {
                    let n = *next.read().await;
                    *next.write().await -= 1;
                    let p = producer.clone();
                    set.spawn(async move {
                        log::info!("next: {}", n);
                        (n, p())
                    });
                }
            }
            anyhow::Ok(())
        });

        let consumer = self.consumer;
        let heap = bin_heap.clone();
        let iterations = self.iterations.clone();
        let current = self.current.clone();
        let jh_consume = spawn(async move {
            loop {
                if *iterations.read().await == 0 {
                    break;
                }
                {
                    let peek = heap.read().await.peek().map(|data| data.idx);
                    if peek.is_none() {
                        sleep(std::time::Duration::from_millis(5)).await;
                        continue;
                    }
                    let cur = current.read().await;
                    if peek.unwrap().lt(&*cur) {
                        sleep(std::time::Duration::from_millis(5)).await;
                        continue;
                    }
                }
                if let Some(data) = heap.write().await.pop() {
                    log::info!("output {:?}", data.idx);
                    consumer(data.value);
                    *current.write().await -= 1;
                    *iterations.write().await -= 1;
                }
            }
        });

        let _ = jh_heap.await?;
        jh_consume.await?;

        Ok(())
    }
}

#[cfg(test)]
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


