use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::js_sys;
use web_sys::{
    HtmlCanvasElement, MouseEvent, WebGlProgram, WebGlRenderingContext as GL,
    WebGlShader, WheelEvent,
};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 20.0;
const ZOOM_SPEED: f32 = 0.001;

#[derive(Clone, Copy)]
struct View {
    zoom: f32,
    x: f32,
    y: f32,
}

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("no window")
        .document()
        .expect("no document")
}

fn compile_shader(
    gl: &GL,
    kind: u32,
    src: &str,
) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| JsValue::from_str("could not create shader"))?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(JsValue::from_str(
            &gl.get_shader_info_log(&shader).unwrap_or_default(),
        ))
    }
}

fn link_program(
    gl: &GL,
    vert: &WebGlShader,
    frag: &WebGlShader,
) -> Result<WebGlProgram, JsValue> {
    let program = gl
        .create_program()
        .ok_or_else(|| JsValue::from_str("could not create program"))?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(JsValue::from_str(
            &gl.get_program_info_log(&program).unwrap_or_default(),
        ))
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let canvas = document()
        .get_element_by_id("canvas")
        .expect("#canvas not found")
        .dyn_into::<HtmlCanvasElement>()?;

    let gl = canvas
        .get_context("webgl")?
        .ok_or_else(|| JsValue::from_str("WebGL not supported"))?
        .dyn_into::<GL>()?;

    let vert = compile_shader(
        &gl,
        GL::VERTEX_SHADER,
        r#"
        attribute vec2 position;
        uniform float zoom;
        uniform vec2 offset;
        void main() {
            gl_Position = vec4(position * zoom + offset, 0.0, 1.0);
        }
        "#,
    )?;
    let frag = compile_shader(
        &gl,
        GL::FRAGMENT_SHADER,
        r#"
        precision mediump float;
        void main() {
            gl_FragColor = vec4(1.0, 0.4, 0.2, 1.0);
        }
        "#,
    )?;
    let program = link_program(&gl, &vert, &frag)?;
    gl.use_program(Some(&program));

    let vertices: [f32; 6] = [-0.7, -0.7, 0.7, -0.7, 0.0, 0.7];

    let buffer = gl
        .create_buffer()
        .ok_or_else(|| JsValue::from_str("could not create buffer"))?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));
    gl.buffer_data_with_array_buffer_view(
        GL::ARRAY_BUFFER,
        &js_sys::Float32Array::from(&vertices[..]),
        GL::STATIC_DRAW,
    );

    let loc = gl.get_attrib_location(&program, "position") as u32;
    gl.vertex_attrib_pointer_with_i32(loc, 2, GL::FLOAT, false, 0, 0);
    gl.enable_vertex_attrib_array(loc);

    let zoom_loc = gl
        .get_uniform_location(&program, "zoom")
        .ok_or_else(|| JsValue::from_str("uniform `zoom` not found"))?;
    let offset_loc = gl
        .get_uniform_location(&program, "offset")
        .ok_or_else(|| JsValue::from_str("uniform `offset` not found"))?;

    let view = Rc::new(Cell::new(View {
        zoom: 1.0,
        x: 0.0,
        y: 0.0,
    }));

    let redraw: Rc<dyn Fn()> = {
        let view = Rc::clone(&view);
        let gl = gl.clone();
        Rc::new(move || {
            let v = view.get();
            gl.uniform1f(Some(&zoom_loc), v.zoom);
            gl.uniform2f(Some(&offset_loc), v.x, v.y);
            gl.clear_color(0.1, 0.1, 0.15, 1.0);
            gl.clear(GL::COLOR_BUFFER_BIT);
            gl.draw_arrays(GL::TRIANGLES, 0, 3);
        })
    };
    redraw();

    let on_wheel = {
        let view = Rc::clone(&view);
        let redraw = Rc::clone(&redraw);
        let canvas = canvas.clone();
        Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            event.prevent_default();

            let rect = canvas.get_bounding_client_rect();
            let cx = ((event.client_x() as f64 - rect.left()) / rect.width()
                * 2.0
                - 1.0) as f32;
            let cy = (1.0
                - (event.client_y() as f64 - rect.top()) / rect.height() * 2.0)
                as f32;

            let v = view.get();
            let factor = (-event.delta_y() as f32 * ZOOM_SPEED).exp();
            let new_zoom = (v.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            let k = new_zoom / v.zoom;

            view.set(View {
                zoom: new_zoom,
                x: cx - (cx - v.x) * k,
                y: cy - (cy - v.y) * k,
            });
            redraw();
        })
    };
    canvas.add_event_listener_with_callback(
        "wheel",
        on_wheel.as_ref().unchecked_ref(),
    )?;
    on_wheel.forget();

    let last_mouse: Rc<Cell<Option<(i32, i32)>>> = Rc::new(Cell::new(None));

    let on_down = {
        let last_mouse = Rc::clone(&last_mouse);
        Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            last_mouse.set(Some((event.client_x(), event.client_y())));
        })
    };
    canvas.add_event_listener_with_callback(
        "mousedown",
        on_down.as_ref().unchecked_ref(),
    )?;
    on_down.forget();

    let window = web_sys::window().expect("no window");

    let on_move = {
        let last_mouse = Rc::clone(&last_mouse);
        let view = Rc::clone(&view);
        let redraw = Rc::clone(&redraw);
        let canvas = canvas.clone();
        Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            if let Some((lx, ly)) = last_mouse.get() {
                let (mx, my) = (event.client_x(), event.client_y());
                let rect = canvas.get_bounding_client_rect();

                let dx = (mx - lx) as f64 / rect.width() * 2.0;
                let dy = (my - ly) as f64 / rect.height() * 2.0;

                let v = view.get();
                view.set(View {
                    zoom: v.zoom,
                    x: v.x + dx as f32,
                    y: v.y - dy as f32,
                });
                last_mouse.set(Some((mx, my)));
                redraw();
            }
        })
    };
    window.add_event_listener_with_callback(
        "mousemove",
        on_move.as_ref().unchecked_ref(),
    )?;
    on_move.forget();

    let on_up = {
        let last_mouse = Rc::clone(&last_mouse);
        Closure::<dyn FnMut(MouseEvent)>::new(move |_event: MouseEvent| {
            last_mouse.set(None);
        })
    };
    window.add_event_listener_with_callback(
        "mouseup",
        on_up.as_ref().unchecked_ref(),
    )?;
    on_up.forget();

    Ok(())
}
