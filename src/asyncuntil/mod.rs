use std::{
    future::Future,
    sync::{Arc, Mutex},
};

pub trait AsyncIterator {
    ///Execute an asynchronous function on multiple tasks.
    ///# Arguments
    ///- `task_num`: Number of tasks to execute the function on.
    ///# Type Parameters
    ///- `F`: The type of future that will be executed. It must implement the `Future` trait and be `Send + 'static`.
    ///# Examples
    ///```
    ///use launcher::asyncuntil::AsyncIterator;
    ///use std::time::Duration;
    ///use tokio::time::sleep;
    ///(0..20).map(|x| async move {
    ///     sleep(Duration::from_secs(1)).await;
    ///     println!("{}",x);
    ///}).async_execute(10);
    ///```
    #[tokio::main(flavor = "current_thread")]
    async fn async_execute<F>(self, task_num: usize)
    where
        Self: Iterator<Item = F> + Sized,
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let mut handles = Vec::with_capacity(task_num);
        let futures = Arc::from(Mutex::from(Vec::from_iter(self)));
        for _ in 0..task_num {
            let futures_share = futures.clone();
            handles.push(tokio::spawn(async move {
                loop {
                    let future = futures_share.lock().unwrap().pop();
                    if let Some(_future) = future {
                        _future.await;
                    } else {
                        break;
                    }
                }
            }))
        }
        for handle in handles {
            handle.await.unwrap();
        }
    }
}

impl<T, F> AsyncIterator for T
where
    T: Iterator<Item = F> + Sized,
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
}
