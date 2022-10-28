use std::sync::Arc;
use anyhow::Result;
use parking_lot::RwLock;
use parallel_runner::{ParallelRunner, ProducerFn};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let num = Arc::new(RwLock::new(5));
    let p: ProducerFn<i32> = Arc::new(move |idx| {
        let n = idx as i32;
        *num.write() += 1;
        std::thread::sleep(std::time::Duration::from_secs(1));
        log::info!("index: {}", idx);
        (idx, n)
    });

    let mut runner = ParallelRunner::new(
        5,
        5,
        Some(23),
        p,
        &|item| { log::info!("eval: {}", item)}
    )?;
    runner.run().await?;
    Ok(())
}