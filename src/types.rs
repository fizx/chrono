use futures::future::Shared;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use wasm_bindgen::prelude::*;

pub type State = HashMap<String, String>;

pub type Closer = Box<dyn FnOnce() + Send>;

pub type BoxedFuture<T, E = String> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;
pub type SharedFuture<T, E = String> = Shared<BoxedFuture<T, E>>;

pub trait Dispatcher {
    fn submit(&self, events: Vec<Event>) -> Result<(), String>;
    fn subscribe(&self, callback: Box<dyn Fn(Response) + Send>) -> Result<Closer, String>;
}

pub type Handler = fn(Request) -> BoxedFuture<Response, String>;

#[derive(Clone, Debug)]
pub struct Event {
    pub remote: bool,
    pub stateful: bool,
    pub data: String,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub state: State,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug)]
pub struct Response {
    pub error_message: Option<String>,
    pub state: State,
    pub replay: Vec<Event>,
    pub data: String,
}
