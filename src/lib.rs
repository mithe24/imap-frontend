mod osm;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::js_sys;
use web_sys::{
    HtmlCanvasElement, MouseEvent, WebGlBuffer, WebGlProgram,
    WebGlRenderingContext as GL, WebGlShader, WheelEvent,
};

const MIN_ZOOM: f32 = 0.4;
const MAX_ZOOM: f32 = 20.0;
const ZOOM_SPEED: f32 = 0.001;
const PAN_LIMIT: f32 = 2.0;

#[derive(Clone, Copy)]
struct View {
    zoom: f32,
    x: f32,
    y: f32,
}

impl View {
    fn is_finite(&self) -> bool {
        self.zoom.is_finite() && self.x.is_finite() && self.y.is_finite()
    }

    fn clamped(self) -> View {
        let zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let limit = PAN_LIMIT * zoom;
        View {
            zoom,
            x: self.x.clamp(-limit, limit),
            y: self.y.clamp(-limit, limit),
        }
    }
}

fn err(msg: impl AsRef<str>) -> JsValue {
    JsValue::from_str(msg.as_ref())
}

fn log_error(msg: impl AsRef<str>) {
    web_sys::console::error_1(&err(msg));
}

fn document() -> Result<web_sys::Document, JsValue> {
    web_sys::window()
        .ok_or_else(|| {
            err("no global `window` (not running in a browser main thread?)")
        })?
        .document()
        .ok_or_else(|| err("window has no `document`"))
}

fn compile_shader(
    gl: &GL,
    kind: u32,
    src: &str,
) -> Result<WebGlShader, JsValue> {
    let label = if kind == GL::VERTEX_SHADER {
        "vertex"
    } else {
        "fragment"
    };

    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| err(format!("could not create {label} shader")))?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let log = gl.get_shader_info_log(&shader).unwrap_or_default();
        gl.delete_shader(Some(&shader));
        Err(err(format!("{label} shader failed to compile: {log}")))
    }
}

fn link_program(
    gl: &GL,
    vert: &WebGlShader,
    frag: &WebGlShader,
) -> Result<WebGlProgram, JsValue> {
    let program = gl
        .create_program()
        .ok_or_else(|| err("could not create shader program"))?;
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
        let log = gl.get_program_info_log(&program).unwrap_or_default();
        gl.delete_program(Some(&program));
        Err(err(format!("shader program failed to link: {log}")))
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let canvas = document()?
        .get_element_by_id("canvas")
        .ok_or_else(|| err("element #canvas not found in the page"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| err("element #canvas exists but is not a <canvas>"))?;

    let gl = canvas
        .get_context("webgl")
        .map_err(|e| err(format!("failed to request a WebGL context: {e:?}")))?
        .ok_or_else(|| {
            err("WebGL is not supported or is disabled in this browser")
        })?
        .dyn_into::<GL>()
        .map_err(|_| err("canvas returned a context that is not WebGL"))?;

    let vert = compile_shader(
        &gl,
        GL::VERTEX_SHADER,
        r#"
        attribute vec2 position;
        uniform float zoom;
        uniform vec2 offset;
        void main() {
            gl_Position = vec4(position * zoom + offset, 0.0, 1.0);
            gl_PointSize = 4.0; 
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
        .ok_or_else(|| err("could not create vertex buffer"))?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));
    gl.buffer_data_with_array_buffer_view(
        GL::ARRAY_BUFFER,
        &js_sys::Float32Array::from(&vertices[..]),
        GL::STATIC_DRAW,
    );

    let position_loc = gl.get_attrib_location(&program, "position");
    if position_loc < 0 {
        return Err(err("attribute `position` not found in shader program"));
    }
    let position_loc = position_loc as u32;
    gl.vertex_attrib_pointer_with_i32(position_loc, 2, GL::FLOAT, false, 0, 0);
    gl.enable_vertex_attrib_array(position_loc);

    let zoom_loc = gl
        .get_uniform_location(&program, "zoom")
        .ok_or_else(|| err("uniform `zoom` not found in shader program"))?;
    let offset_loc = gl
        .get_uniform_location(&program, "offset")
        .ok_or_else(|| err("uniform `offset` not found in shader program"))?;

    let points: Rc<RefCell<Option<(WebGlBuffer, i32)>>> = Rc::new(RefCell::new(None));

    let view = Rc::new(Cell::new(View {
        zoom: 1.0,
        x: 0.0,
        y: 0.0,
    }));

    let redraw: Rc<dyn Fn()> = {
        let view = Rc::clone(&view);
        let gl = gl.clone();
        let buffer = buffer.clone();
        let points = Rc::clone(&points);
        Rc::new(move || {
            if gl.is_context_lost() {
                log_error(
                    "WebGL context lost; skipping draw (reload the page to recover)",
                );
                return;
            }

            let v = view.get();
            gl.uniform1f(Some(&zoom_loc), v.zoom);
            gl.uniform2f(Some(&offset_loc), v.x, v.y);
            gl.clear_color(0.1, 0.1, 0.15, 1.0);
            gl.clear(GL::COLOR_BUFFER_BIT);

            gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));
            gl.vertex_attrib_pointer_with_i32(
                position_loc,
                2,
                GL::FLOAT,
                false,
                0,
                0,
            );
            gl.draw_arrays(GL::TRIANGLES, 0, 3);

            if let Some((points_buffer, count)) = points.borrow().as_ref() {
                gl.bind_buffer(GL::ARRAY_BUFFER, Some(points_buffer));
                gl.vertex_attrib_pointer_with_i32(
                    position_loc,
                    2,
                    GL::FLOAT,
                    false,
                    0,
                    0,
                );
                gl.draw_arrays(GL::POINTS, 0, *count);
            }

            let code = gl.get_error();
            if code != GL::NO_ERROR {
                log_error(format!("WebGL error after draw: 0x{code:X}"));
            }
        })
    };
    redraw();

    // temporary draw of the points from the osm fetch
    // flatten the points, create buffer and bind to gl
    // update and mutate points for redraw to redraw
    {
        let gl = gl.clone();
        let points = Rc::clone(&points);
        let redraw = Rc::clone(&redraw);
        wasm_bindgen_futures::spawn_local(async move {
            match osm::fetch_sdu_map_data().await {
                Ok(ways) => {
                    let flat = osm::flatten_points(&ways);
                    let count = (flat.len() / 2) as i32;

                    let points_buffer = match gl.create_buffer() {
                        Some(b) => b,
                        None => {
                            log_error("could not create points buffer");
                            return;
                        }
                    };
                    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&points_buffer));
                    gl.buffer_data_with_array_buffer_view(
                        GL::ARRAY_BUFFER,
                        &js_sys::Float32Array::from(&flat[..]),
                        GL::STATIC_DRAW,
                    );

                    web_sys::console::log_1(
                        &format!(
                            "osm: fetched {} ways, {count} points",
                            ways.len()
                        )
                        .into(),
                    );

                    *points.borrow_mut() = Some((points_buffer, count));
                    redraw();
                }
                Err(e) => {
                    web_sys::console::error_1(&e);
                }
            }
        });
    }

    let on_wheel = {
        let view = Rc::clone(&view);
        let redraw = Rc::clone(&redraw);
        let canvas = canvas.clone();
        Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            event.prevent_default();

            let rect = canvas.get_bounding_client_rect();
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                return;
            }
            let delta = event.delta_y() as f32;
            if !delta.is_finite() {
                return;
            }

            let cx = ((event.client_x() as f64 - rect.left()) / rect.width()
                * 2.0
                - 1.0) as f32;
            let cy = (1.0
                - (event.client_y() as f64 - rect.top()) / rect.height() * 2.0)
                as f32;

            let v = view.get();
            let new_zoom = (v.zoom * (-delta * ZOOM_SPEED).exp())
                .clamp(MIN_ZOOM, MAX_ZOOM);
            let k = new_zoom / v.zoom;

            let next = View {
                zoom: new_zoom,
                x: cx - (cx - v.x) * k,
                y: cy - (cy - v.y) * k,
            };
            if !next.is_finite() {
                log_error(
                    "ignored wheel event that produced a non-finite view",
                );
                return;
            }

            view.set(next.clamped());
            redraw();
        })
    };
    canvas
        .add_event_listener_with_callback(
            "wheel",
            on_wheel.as_ref().unchecked_ref(),
        )
        .map_err(|e| err(format!("failed to attach wheel listener: {e:?}")))?;
    on_wheel.forget();

    let last_mouse: Rc<Cell<Option<(i32, i32)>>> = Rc::new(Cell::new(None));

    let on_down = {
        let last_mouse = Rc::clone(&last_mouse);
        Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            last_mouse.set(Some((event.client_x(), event.client_y())));
        })
    };
    canvas
        .add_event_listener_with_callback(
            "mousedown",
            on_down.as_ref().unchecked_ref(),
        )
        .map_err(|e| {
            err(format!("failed to attach mousedown listener: {e:?}"))
        })?;
    on_down.forget();

    let window = web_sys::window().ok_or_else(|| err("no global `window`"))?;

    let on_move = {
        let last_mouse = Rc::clone(&last_mouse);
        let view = Rc::clone(&view);
        let redraw = Rc::clone(&redraw);
        let canvas = canvas.clone();
        Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let Some((lx, ly)) = last_mouse.get() else {
                return;
            };

            if event.buttons() & 1 == 0 {
                last_mouse.set(None);
                return;
            }

            let rect = canvas.get_bounding_client_rect();
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                return;
            }

            let (mx, my) = (event.client_x(), event.client_y());
            let dx = ((mx - lx) as f64 / rect.width() * 2.0) as f32;
            let dy = ((my - ly) as f64 / rect.height() * 2.0) as f32;

            let v = view.get();
            let next = View {
                zoom: v.zoom,
                x: v.x + dx,
                y: v.y - dy,
            };
            if !next.is_finite() {
                log_error("ignored mouse move that produced a non-finite view");
                return;
            }

            view.set(next.clamped());
            last_mouse.set(Some((mx, my)));
            redraw();
        })
    };
    window
        .add_event_listener_with_callback(
            "mousemove",
            on_move.as_ref().unchecked_ref(),
        )
        .map_err(|e| {
            err(format!("failed to attach mousemove listener: {e:?}"))
        })?;
    on_move.forget();

    let on_up = {
        let last_mouse = Rc::clone(&last_mouse);
        Closure::<dyn FnMut(MouseEvent)>::new(move |_event: MouseEvent| {
            last_mouse.set(None);
        })
    };
    window
        .add_event_listener_with_callback(
            "mouseup",
            on_up.as_ref().unchecked_ref(),
        )
        .map_err(|e| {
            err(format!("failed to attach mouseup listener: {e:?}"))
        })?;
    on_up.forget();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_is_finite_for_normal_values() {
        let view = View {
            zoom: 1.0,
            x: 0.0,
            y: 0.0,
        };

        assert!(view.is_finite());
    }

    #[test]
    fn view_is_not_finite_for_nan_zoom() {
        let view = View {
            zoom: f32::NAN,
            x: 0.0,
            y: 0.0,
        };

        assert!(!view.is_finite());
    }

    #[test]
    fn view_is_not_finite_for_infinite_x() {
        let view = View {
            zoom: 1.0,
            x: f32::INFINITY,
            y: 0.0,
        };

        assert!(!view.is_finite());
    }

    #[test]
    fn view_is_not_finite_for_negative_infinite_y() {
        let view = View {
            zoom: 1.0,
            x: 0.0,
            y: f32::NEG_INFINITY,
        };

        assert!(!view.is_finite());
    }

    #[test]
    fn zoom_is_clamped_to_minimum() {
        let view = View {
            zoom: 0.1,
            x: 0.0,
            y: 0.0,
        }
        .clamped();

        assert_eq!(view.zoom, MIN_ZOOM);
    }

    #[test]
    fn zoom_is_clamped_to_maximum() {
        let view = View {
            zoom: 100.0,
            x: 0.0,
            y: 0.0,
        }
        .clamped();

        assert_eq!(view.zoom, MAX_ZOOM);
    }

    #[test]
    fn zoom_inside_range_is_unchanged() {
        let view = View {
            zoom: 2.0,
            x: 0.0,
            y: 0.0,
        }
        .clamped();

        assert_eq!(view.zoom, 2.0);
    }

    #[test]
    fn pan_is_clamped_based_on_zoom() {
        let view = View {
            zoom: 2.0,
            x: 100.0,
            y: -100.0,
        }
        .clamped();

        let limit = PAN_LIMIT * 2.0;

        assert_eq!(view.x, limit);
        assert_eq!(view.y, -limit);
    }

    #[test]
    fn pan_inside_limit_is_unchanged() {
        let view = View {
            zoom: 2.0,
            x: 3.0,
            y: -3.0,
        }
        .clamped();

        assert_eq!(view.x, 3.0);
        assert_eq!(view.y, -3.0);
    }

    #[test]
    fn pan_limit_changes_with_zoom() {
        let view = View {
            zoom: 0.5,
            x: 10.0,
            y: -10.0,
        }
        .clamped();

        let limit = PAN_LIMIT * 0.5;

        assert_eq!(view.zoom, 0.5);
        assert_eq!(view.x, limit);
        assert_eq!(view.y, -limit);
    }

    #[test]
    fn clamped_view_remains_finite() {
        let view = View {
            zoom: 1.0,
            x: 1.0,
            y: -1.0,
        }
        .clamped();

        assert!(view.is_finite());
    }

    #[test]
    fn minimum_zoom_has_expected_pan_limit() {
        let view = View {
            zoom: MIN_ZOOM,
            x: 100.0,
            y: -100.0,
        }
        .clamped();

        let expected_limit = PAN_LIMIT * MIN_ZOOM;

        assert_eq!(view.x, expected_limit);
        assert_eq!(view.y, -expected_limit);
    }

    #[test]
    fn maximum_zoom_has_expected_pan_limit() {
        let view = View {
            zoom: MAX_ZOOM,
            x: 100.0,
            y: -100.0,
        }
        .clamped();

        let expected_limit = PAN_LIMIT * MAX_ZOOM;

        assert_eq!(view.x, expected_limit);
        assert_eq!(view.y, -expected_limit);
    }

    #[test]
    fn view_can_be_copied() {
        let original = View {
            zoom: 2.0,
            x: 1.0,
            y: -1.0,
        };

        let copy = original;

        assert_eq!(original.zoom, copy.zoom);
        assert_eq!(original.x, copy.x);
        assert_eq!(original.y, copy.y);
    }
}
