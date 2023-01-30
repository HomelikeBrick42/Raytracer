use std::io::Write;

use rand::Rng;
use rayon::{
    prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use raytracer::{
    get_closest_object, ray_trace, Camera, Material, Object, BOUNCES, SAMPLES_PER_BOUNCE,
};
use thallium::{
    math::{Matrix4x4, One, Vector2, Vector3, Zero},
    platform::{Keycode, MouseButton, Surface, SurfaceEvent},
    renderer::{Pixels, PrimitiveType, RendererAPI, VertexBufferElement},
    slice_to_bytes,
};

fn main() {
    let mut renderer =
        Surface::new((640, 480).into(), "Ray tracer").into_renderer(RendererAPI::OpenGL);

    let shader = renderer
        .create_shader(
            r#"#version 410 core
layout(location = 0) in vec2 a_Position;
layout(location = 1) in vec2 a_TexCoord;

out vec2 u_TexCoord;

void main() {
    gl_Position = vec4(a_Position, 0.0, 1.0);
    u_TexCoord = a_TexCoord;
}
"#,
            r#"#version 410 core
layout(location = 0) out vec4 o_Color;

in vec2 u_TexCoord;

uniform vec3 u_Color;
uniform sampler2D u_Texture;

void main() {
    o_Color = vec4(u_Color, 1.0) * texture(u_Texture, u_TexCoord);
}
"#,
        )
        .unwrap();

    let vertex_buffer = {
        #[repr(C)]
        struct Vertex {
            position: Vector2<f32>,
            tex_coord: Vector2<f32>,
        }
        let vertices: &[Vertex] = &[
            Vertex {
                position: (-1.0, -1.0).into(),
                tex_coord: (0.0, 0.0).into(),
            },
            Vertex {
                position: (-1.0, 1.0).into(),
                tex_coord: (0.0, 1.0).into(),
            },
            Vertex {
                position: (1.0, -1.0).into(),
                tex_coord: (1.0, 0.0).into(),
            },
            Vertex {
                position: (1.0, 1.0).into(),
                tex_coord: (1.0, 1.0).into(),
            },
        ];

        renderer.create_vertex_buffer(
            &[VertexBufferElement::Float2, VertexBufferElement::Float2],
            slice_to_bytes(vertices),
        )
    };

    let (mut pixels, texture) = {
        let size @ Vector2 {
            x: width,
            y: height,
        } = renderer.get_surface_mut().get_size();
        let pixels = vec![Vector3::zero(); width * height];
        let texture = renderer.create_texture(size, Pixels::RGBF(&pixels));
        (pixels, texture)
    };

    let mut camera = Camera {
        position: (0.0, 1.4, -2.0).into(),
        right: (1.0, 0.0, 0.0).into(),
        up: (0.0, 1.0, 0.0).into(),
        forward: (0.0, 0.0, 1.0).into(),
    };

    let mut objects = vec![
        Object::Plane {
            normal: (0.0, 1.0, 0.0).into(),
            distance_along_normal: 0.0,
            material: Material {
                diffuse_color: (0.2, 0.8, 0.3).into(),
                emit_color: (0.0, 0.0, 0.0).into(),
                reflectiveness: 0.0,
            },
        },
        Object::Sphere {
            center: (-1.0, 1.0, 0.0).into(),
            radius: 1.0,
            material: Material {
                diffuse_color: (0.8, 0.3, 0.2).into(),
                emit_color: (0.0, 0.0, 0.0).into(),
                reflectiveness: 0.0,
            },
        },
        Object::Sphere {
            center: (1.5, 1.0, 0.0).into(),
            radius: 1.0,
            material: Material {
                diffuse_color: (0.95, 0.95, 0.95).into(),
                emit_color: (0.0, 0.0, 0.0).into(),
                reflectiveness: 0.95,
            },
        },
    ];

    let mut frames_since_movement = 0usize;

    renderer.get_surface_mut().show();
    let mut last_time = std::time::Instant::now();
    'main_loop: loop {
        let time = std::time::Instant::now();
        let dt = time.duration_since(last_time).as_secs_f32();
        last_time = time;

        print!("{:.3}ms          \r", dt * 1000.0);
        std::io::stdout().flush().unwrap();

        let size @ Vector2 {
            x: width,
            y: height,
        } = renderer.get_surface_mut().get_size();
        let aspect = width as f32 / height as f32;

        for event in renderer.get_surface_mut().events() {
            match event {
                SurfaceEvent::Close => break 'main_loop,
                SurfaceEvent::Resize(
                    size @ Vector2 {
                        x: width,
                        y: height,
                    },
                ) => {
                    renderer.resize(size);
                    pixels = vec![Vector3::zero(); width * height];
                    frames_since_movement = 0;
                }
                SurfaceEvent::MousePressed(button, Vector2 { x, y }) => 'mouse_press_handling: {
                    if x < 0 && x >= width as isize && y < 0 && y >= height as isize {
                        break 'mouse_press_handling;
                    }

                    let coord = (x as usize, height - y as usize - 1).into();
                    let uv = Camera::get_uv(coord, size);
                    let ray = camera.get_ray(uv, aspect);

                    match button {
                        MouseButton::Left => {
                            if let Some(hit) = get_closest_object(ray, &objects) {
                                objects.push(Object::Sphere {
                                    center: hit.position + hit.normal * 0.5.into(),
                                    radius: 0.5,
                                    material: Material {
                                        diffuse_color: (0.0, 0.0, 0.0).into(),
                                        emit_color: (3.0, 3.0, 3.0).into(),
                                        reflectiveness: 0.0,
                                    },
                                });
                                frames_since_movement = 0;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Update
        {
            let mut moved = false;
            if renderer.get_surface().get_key_state(Keycode::W) {
                camera.position += camera.forward * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::S) {
                camera.position -= camera.forward * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::A) {
                camera.position -= camera.right * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::D) {
                camera.position += camera.right * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::Q) {
                camera.position -= camera.up * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::E) {
                camera.position += camera.up * dt.into();
                moved = true;
            }

            if moved {
                frames_since_movement = 0;
            }
        }

        // Ray trace stuff
        {
            if frames_since_movement == 0 {
                pixels.fill(Vector3::zero());
            }

            pixels
                .par_chunks_mut(width)
                .enumerate()
                .flat_map(|(y, row)| {
                    row.par_iter_mut()
                        .enumerate()
                        .map(move |(x, pixel)| (x, y, pixel))
                })
                .for_each(|(x, y, pixel)| {
                    let mut rng = rand::thread_rng();

                    let mut color = Vector3::<f32>::zero();
                    for _ in 0..SAMPLES_PER_BOUNCE {
                        let uv = Vector2 {
                            x: (x as f32 + rng.gen::<f32>() * 2.0 - 1.0) / width as f32,
                            y: (y as f32 + rng.gen::<f32>() * 2.0 - 1.0) / height as f32,
                        };
                        let ray = camera.get_ray(uv, aspect);
                        color += ray_trace(ray, &objects, &mut rng, BOUNCES);
                    }
                    color *= (1.0 / SAMPLES_PER_BOUNCE as f32).into();

                    *pixel += (color - *pixel) / (frames_since_movement as f32 + 1.0).into();
                });
        }

        // Render to window
        renderer.clear(Vector3::zero());
        {
            renderer
                .get_texture_mut(texture)
                .unwrap()
                .set_pixels(size, Pixels::RGBF(&pixels));
            let mut draw_context = renderer.drawing_context(Default::default(), false);
            draw_context.draw(
                PrimitiveType::TriangleStrip,
                shader,
                vertex_buffer,
                Some(texture),
                Matrix4x4::default(),
                Vector3::one(),
            );
        }
        renderer.present();

        frames_since_movement += 1;
    }
    renderer.get_surface_mut().hide();
}
