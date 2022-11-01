use std::sync::Arc;
use anyhow::Result;
use parking_lot::RwLock;
use rand::Rng;
use parallel_runner::{ParallelRunner, ProducerFn};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let num = Arc::new(RwLock::new(5));

    // producer function
    let p: ProducerFn<i32> = Arc::new(move |idx| {
        let payload = idx as i32;

        *num.write() += 1;
        
        log::info!("pr: ------------< {}", idx);

        // the sleep is simulating the processing of the async task
        // sleeps a random amount of time
        std::thread::sleep(std::time::Duration::from_millis(1000 + rand::thread_rng().gen_range(333..3333)));

        (idx, payload)
    });


    // initializing the runner
    let mut runner = ParallelRunner::new(
        5, // the amount of parellel processes
        5, // starting index
        Some(23), // size of processing

        // inject producer function
        p, 

        // inject consumer function
        &|item| { log::info!("pr: .................. {}", item) }
    )?;

    // here we execute the process and wait for finishing
    runner.run().await?;

    Ok(())
}