use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlElement;

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("no window")
        .document()
        .expect("no document")
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let doc = document();
    let button = doc.get_element_by_id("btn").expect("#btn not found");
    let label = doc.get_element_by_id("count").expect("#count not found");

    let count = Rc::new(Cell::new(0u32));

    let on_click = {
        let count = Rc::clone(&count);
        Closure::<dyn FnMut()>::new(move || {
            count.set(count.get() + 1);
            label.set_text_content(Some(&format!(
                "Clicked {} times",
                count.get()
            )));
        })
    };

    button.add_event_listener_with_callback(
        "click",
        on_click.as_ref().unchecked_ref(),
    )?;

    // Hand ownership of the closure to JS
    // so it isn't dropped when start() returns.
    on_click.forget();
    Ok(())
}

/// Programmatically clicks the button `n` times.
/// Each call fires the same click handler as a real user click.
#[wasm_bindgen]
pub fn click_n_times(n: u32) -> Result<(), JsValue> {
    let button = document()
        .get_element_by_id("btn")
        .expect("#btn not found")
        .dyn_into::<HtmlElement>()?;

    for _ in 0..n {
        button.click();
    }
    Ok(())
}
