use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::js_sys;
use web_sys::{
    HtmlCanvasElement, WebGlProgram, WebGlRenderingContext as GL, WebGlShader,
};

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
        void main() {
            gl_Position = vec4(position, 0.0, 1.0);
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

    let vertices: [f32; 6] = [
        -0.7, -0.7,
        0.7, -0.7,
        0.0, 0.7,
    ];

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

    gl.clear_color(0.1, 0.1, 0.15, 1.0);
    gl.clear(GL::COLOR_BUFFER_BIT);
    gl.draw_arrays(GL::TRIANGLES, 0, 3);

    Ok(())
}
