use std::iter::Map;

pub type State = Map<String, String>;

pub type Closer = fn() -> ();

pub trait Dispatcher {
    fn submit(&self, events: Vec<Event>) -> Result<(), String>;
    fn subscribe(&self, callback: fn(Response) -> ()) -> Result<Closer, String>;
}

pub type Handler = fn(Request) -> Response;

pub struct Event {
    pub remote: bool,
    pub stateful: bool,
    pub data: String,
}
pub struct Effect {
    pub data: String,
}

pub struct Request {
    pub state: State,
    pub events: Vec<Event>,
}
pub struct Response {
    pub error_message: Option<String>,
    pub state: State,
    pub replay: Vec<Event>,
    pub effects: Vec<Effect>,
}
