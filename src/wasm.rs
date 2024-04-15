use std::{collections::HashMap, sync::Arc};

use crate::{
    fallback::{FallbackDispatcher, FallbackDispatcherOptions, SingleThreadedAsyncStdExecutor},
    types::{BoxedFuture, Handler, Request, Response},
};
use futures::future::FutureExt;
use js_sys::Function;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct JsEvent {
    pub remote: bool,
    pub stateful: bool,
    data: js_sys::Uint8Array, // pub data: std::sync::Arc<String>,
}

#[wasm_bindgen]
impl JsEvent {
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> js_sys::Uint8Array {
        self.data.clone()
    }
}

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct JsRequest {
    state: JsValue, // Assuming State has complex or non-primitive fields
    events: Vec<JsEvent>,
}

#[wasm_bindgen]
impl JsRequest {
    #[wasm_bindgen(getter)]
    pub fn state(&self) -> JsValue {
        self.state.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn events(&self) -> Vec<JsEvent> {
        self.events.clone()
    }
}

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct JsResponse {
    error_message: Option<String>,
    state: JsValue, // As above, using JsValue to encapsulate complex State
    replay: Vec<JsEvent>,
    data: js_sys::Uint8Array,
}

#[wasm_bindgen]
impl JsResponse {
    #[wasm_bindgen(getter)]
    pub fn error_message(&self) -> Option<String> {
        self.error_message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn state(&self) -> JsValue {
        self.state.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn replay(&self) -> Vec<JsEvent> {
        self.replay.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> js_sys::Uint8Array {
        self.data.clone()
    }
}

pub struct JsHandler {
    js_function: Function,
}
pub fn new_js_handler(js_value: JsValue) -> Result<JsHandler, JsValue> {
    let js_function = js_value
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("Provided JsValue is not a function"))?;
    Ok(JsHandler { js_function })
}

impl Handler for JsHandler {
    fn handle(&self, req: Request) -> BoxedFuture<Response, String> {
        let js_function = self.js_function.clone();

        async move {
            // Convert the request to a JsValue using serde_wasm_bindgen
            let js_request = to_value(&req).unwrap();
            // Call the JS function and await its promise
            let js_result = js_function
                .dyn_into::<js_sys::Function>()
                .unwrap()
                .call1(&JsValue::NULL, &js_request)
                .unwrap();
            let promise: js_sys::Promise = js_result.into();
            let js_future = wasm_bindgen_futures::JsFuture::from(promise);
            let result = js_future.await;

            match result {
                Ok(js_value) => {
                    // Convert the result from JsValue back to a Rust Response struct
                    from_value::<Response>(js_value).map_err(|e| e.to_string())
                }
                Err(e) => Err(e.as_string().unwrap_or_else(|| "Unknown error".to_string())),
            }
        }
        .boxed_local()
    }
}

#[wasm_bindgen]
struct JsDispatcher {
    dispatcher: FallbackDispatcher,
}

#[wasm_bindgen]
impl JsDispatcher {
    #[wasm_bindgen(constructor)]
    pub fn new(local_js: JsValue, remote_js: JsValue) -> JsDispatcher {
        let local_handler = Arc::new(new_js_handler(local_js).unwrap());
        let remote_handler = Arc::new(new_js_handler(remote_js).unwrap());
        let state = HashMap::new(); // Initialize state as per your implementation
        let dispatcher = FallbackDispatcher::new(
            Arc::new(SingleThreadedAsyncStdExecutor),
            state.clone(),
            local_handler,
            remote_handler,
            FallbackDispatcherOptions::default(),
        );
        JsDispatcher { dispatcher }
    }
    #[wasm_bindgen]
    pub fn submit(&self, events: Vec<JsEvent>) {}
    // fn subscribe(&self, callback: Box<dyn Fn(Response) + Send>) -> Result<Closer, String>;
}
