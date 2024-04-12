use wasm_bindgen::prelude::*;

use crate::{
    fallback::FallbackDispatcherOptions,
    types::{Handler, Request, State},
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct JsEvent {
    pub remote: bool,
    pub stateful: bool,
    data: String, // pub data: std::sync::Arc<String>,
}

#[wasm_bindgen]
impl JsEvent {
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> String {
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
    data: String,
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
    pub fn data(&self) -> String {
        self.data.clone()
    }
}

#[wasm_bindgen]
pub struct JsDispatcher {
    dispatcher: crate::fallback::FallbackDispatcher,
}

#[wasm_bindgen]
impl JsDispatcher {
    #[wasm_bindgen(constructor)]
    pub fn new(local_handler: JsValue, remote_handler: JsValue) -> JsDispatcher {
        // Validate that the provided JsValues are indeed functions
        assert!(
            local_handler.is_function(),
            "local_handler must be a function"
        );
        assert!(
            remote_handler.is_function(),
            "remote_handler must be a function"
        );

        let state = State::default(); // Ensure State has a default or an initial state is provided
        let options = FallbackDispatcherOptions::default(); // Assuming it has defaults

        // Convert JsValue to async Rust Handler
        let make_handler = |js_func: JsValue| -> Handler {
            Box::new(move |request: Request| {
                let promise = js_func
                    .call1(&JsValue::NULL, &JsValue::from_serde(&request).unwrap())
                    .map_err(|e| format!("JS call failed: {:?}", e))
                    .and_then(JsFuture::from)
                    .and_then(|resp| {
                        resp.into_serde::<Response>()
                            .map_err(|e| format!("Deserialization failed: {:?}", e))
                    })
                    .boxed_local();

                let pinned_box: BoxedFuture<Response, String> = Box::pin(promise);
                pinned_box
            })
        };
    }
}
