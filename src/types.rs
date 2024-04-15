use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

pub type State = HashMap<String, String>;

pub type Closer = Box<dyn FnOnce() + Send>;

pub type BoxedFuture<T, E = String> = Pin<Box<dyn Future<Output = Result<T, E>>>>;

pub trait Dispatcher {
    fn submit(&self, events: Vec<Event>) -> Result<(), String>;
    fn subscribe(&self, callback: Box<dyn Fn(Response) + Send>) -> Result<Closer, String>;
}

pub trait Handler {
    fn handle(&self, req: Request) -> BoxedFuture<Response, String>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub remote: bool,
    pub stateful: bool,
    pub data: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub state: State,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub error_message: Option<String>,
    pub state: State,
    pub replay: Vec<Event>,
    pub data: String,
}

pub trait Executor {
    fn spawn(&self, fut: BoxedFuture<()>);
}
