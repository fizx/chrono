use crate::types::{
    BoxedFuture, Closer, Dispatcher, Event, Executor, Handler, Request, Response, State,
};

use async_std::prelude::*;
use async_std::task;
use async_std::task_local;
use futures::executor;
use futures::future::{BoxFuture, FutureExt};
use std::{
    cmp::min,
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct FallbackDispatcherOptions {
    pub sync_timeout_millis: u32,
    pub async_timeout_millis: u32,
    pub max_async_concurrency: u32,
    pub async_batch_size: u32,
    pub sync_batch_size: u32,
}

// defaults
impl Default for FallbackDispatcherOptions {
    fn default() -> Self {
        FallbackDispatcherOptions {
            sync_timeout_millis: 1000,
            async_timeout_millis: 1000,
            max_async_concurrency: 10,
            async_batch_size: 10,
            sync_batch_size: 10,
        }
    }
}
#[derive(Clone)]
pub struct FallbackDispatcher {
    executor: Arc<dyn Executor>,
    options: FallbackDispatcherOptions,
    state: State,
    local_handler: Arc<dyn Handler>,
    remote_handler: Arc<dyn Handler>,
    sync_queue: Arc<Mutex<Vec<Event>>>,
    async_queue: Arc<Mutex<Vec<Event>>>,
    async_in_progress: Arc<Mutex<u32>>,
    sync_in_progress: Arc<Mutex<u32>>,
    subscribers: Arc<Mutex<HashMap<u64, Box<dyn Fn(Response) + Send>>>>,
    next_subscriber_id: Arc<Mutex<u64>>,
}

impl FallbackDispatcher {
    pub fn new(
        executor: Arc<dyn Executor>,
        state: State,
        local_handler: Arc<dyn Handler>,
        remote_handler: Arc<dyn Handler>,
        options: FallbackDispatcherOptions,
    ) -> FallbackDispatcher {
        FallbackDispatcher {
            executor,
            options,
            state,
            local_handler,
            remote_handler,
            sync_queue: Arc::new(Mutex::new(Vec::new())),
            async_queue: Arc::new(Mutex::new(Vec::new())),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            async_in_progress: Arc::new(Mutex::new(0)),
            sync_in_progress: Arc::new(Mutex::new(0)),
            next_subscriber_id: Arc::new(Mutex::new(0)),
        }
    }
    fn notify_subscribers(&self, response: Response) {
        let subscribers = self.subscribers.lock().unwrap();
        for subscriber in subscribers.values() {
            subscriber(response.clone());
        }
    }

    async fn handle_request(&self, events: Vec<Event>) {
        // let response = (self.local_handler)(request.clone());
        // self.state = response.state.clone();

        // let mut replay = response.replay.clone();
        // if !request.events.is_empty() {
        //     let response = (self.remote_handler)(request);
        //     self.state = response.state.clone();
        //     replay.extend(response.replay);
        // }

        // self.notify_subscribers(Response {
        //     error_message: None,
        //     state: self.state.clone(),
        //     replay,
        //     data: "".to_string(),
        // });

        // response
        let rsp = Response {
            error_message: None,
            state: self.state.clone(),
            replay: Vec::new(),
            data: "".to_string(),
        };
        self.notify_subscribers(rsp);
    }

    fn tick(&self) {
        let mut async_queue = self.async_queue.lock().unwrap();
        let mut sync_queue = self.sync_queue.lock().unwrap();

        while *self.async_in_progress.lock().unwrap() < self.options.max_async_concurrency {
            //take batch size elements
            let size = min(self.options.async_batch_size as usize, async_queue.len());
            let batch = async_queue.drain(..size).collect::<Vec<Event>>();
            if batch.is_empty() {
                break;
            }
            let self_clone = self.clone();
            *self.async_in_progress.lock().unwrap() += 1;
            self.executor.spawn(
                async move {
                    self_clone.handle_request(batch).await;
                    *self_clone.async_in_progress.lock().unwrap() -= 1;
                    self_clone.tick();
                    Ok(())
                }
                .boxed_local(),
            )
        }

        while *self.sync_in_progress.lock().unwrap() < 1 {
            //take batch size elements
            let size = min(self.options.sync_batch_size as usize, sync_queue.len());
            let batch = sync_queue.drain(..size).collect::<Vec<Event>>();
            if batch.is_empty() {
                break;
            }
            let self_clone = self.clone();
            *self.sync_in_progress.lock().unwrap() += 1;
            self.executor.spawn(
                async move {
                    self_clone.handle_request(batch).await;
                    *self_clone.sync_in_progress.lock().unwrap() -= 1;
                    self_clone.tick();
                    Ok(())
                }
                .boxed_local(),
            );
        }
    }
}

impl Dispatcher for FallbackDispatcher {
    fn submit(&self, events: Vec<Event>) -> Result<(), String> {
        for event in events {
            if event.stateful {
                self.sync_queue.lock().unwrap().push(event);
            } else {
                self.async_queue.lock().unwrap().push(event);
            }
        }
        self.tick();
        Ok(())
    }

    fn subscribe(&self, callback: Box<dyn Fn(Response) + Send>) -> Result<Closer, String> {
        let mut subscribers = self.subscribers.lock().unwrap();
        let mut id_generator = self.next_subscriber_id.lock().unwrap();
        let subscriber_id = *id_generator;
        *id_generator += 1;

        subscribers.insert(subscriber_id, callback);

        let subscribers_clone = Arc::clone(&self.subscribers);
        Ok(Box::new(move || {
            subscribers_clone.lock().unwrap().remove(&subscriber_id);
        }))
    }
}
struct TestHandler {
    pub prefix: String,
}

impl Handler for TestHandler {
    fn handle(&self, req: Request) -> BoxedFuture<Response, String> {
        let p = self.prefix.clone();
        async move {
            let cloned_state = req.state.clone();
            let cloned_events = req.events.clone();
            Ok(Response {
                error_message: None,
                state: cloned_state,
                replay: vec![],
                data: format!("{}: {:?}", p, cloned_events),
            })
        }
        .boxed_local()
    }
}

pub struct SingleThreadedAsyncStdExecutor;

impl Executor for SingleThreadedAsyncStdExecutor {
    fn spawn(&self, fut: BoxedFuture<()>) {
        task::spawn_local(fut.map(|_| ()));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::types::BoxedFuture;

    use super::*;

    // Simulated handlers for testing

    #[async_std::test]
    async fn test_fallback_dispatcher() {
        let local_handler = TestHandler {
            prefix: "handled locally".to_string(),
        };
        let remote_handler = TestHandler {
            prefix: "handled locally".to_string(),
        };

        let state = HashMap::new(); // Initialize state as per your implementation
        let dispatcher = FallbackDispatcher::new(
            Arc::new(SingleThreadedAsyncStdExecutor),
            state.clone(),
            Arc::new(local_handler),
            Arc::new(remote_handler),
            FallbackDispatcherOptions::default(),
        );

        let events = vec![
            Event {
                stateful: true,
                remote: false,
                data: "event1".to_string(),
            },
            Event {
                stateful: true,
                remote: false,
                data: "event2".to_string(),
            },
        ];

        // Subscribe to monitor responses
        let received_responses = Arc::new(Mutex::new(vec![]));
        let responses_clone = received_responses.clone();
        dispatcher
            .subscribe(Box::new(move |response| {
                responses_clone.lock().unwrap().push(response);
            }))
            .unwrap();

        // Submit events to the dispatcher
        dispatcher.submit(events).unwrap();

        // Allow some time for async processing
        task::sleep(Duration::from_millis(10)).await;

        // Check results in received_responses
        let locked_responses = received_responses.lock().unwrap();
        assert!(
            !locked_responses.is_empty(),
            "Responses should not be empty."
        );
        // Add more specific checks as needed

        // Check state consistency and correctness of processing
        assert_eq!(dispatcher.state, state, "State should remain consistent.");

        // Further checks can include specifics of response contents
    }
}
