use std::io::Write;

use rand::Rng;
use rayon::{
    prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use raytracer::{Hit, Material, Object, Ray};
use thallium::{
    math::{Matrix4x4, One, Vector2, Vector3, Zero},
    platform::{Keycode, Surface, SurfaceEvent},
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

    const SAMPLES_PER_BOUNCE: usize = 2;
    const BOUNCES: usize = 5;

    let mut camera_position: Vector3<f32> = (0.0, 1.0, -3.0).into();
    let camera_right: Vector3<f32> = (1.0, 0.0, 0.0).into();
    let camera_up: Vector3<f32> = (0.0, 1.0, 0.0).into();
    let camera_forward: Vector3<f32> = (0.0, 0.0, 1.0).into();
    let objects = [
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
            center: (0.0, 1.0, 0.0).into(),
            radius: (1.0),
            material: Material {
                diffuse_color: (0.8, 0.3, 0.2).into(),
                emit_color: (0.0, 0.0, 0.0).into(),
                reflectiveness: 0.0,
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
                _ => {}
            }
        }

        let size @ Vector2 {
            x: width,
            y: height,
        } = renderer.get_surface_mut().get_size();
        let aspect = width as f32 / height as f32;

        // Update
        {
            let mut moved = false;
            if renderer.get_surface().get_key_state(Keycode::W) {
                camera_position += camera_forward * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::S) {
                camera_position -= camera_forward * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::A) {
                camera_position -= camera_right * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::D) {
                camera_position += camera_right * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::Q) {
                camera_position -= camera_up * dt.into();
                moved = true;
            }
            if renderer.get_surface().get_key_state(Keycode::E) {
                camera_position += camera_up * dt.into();
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
                    fn get_closest_object(ray: Ray, objects: &[Object]) -> Option<Hit> {
                        objects.iter().fold(None, |hit, object| {
                            let new_hit = object.intersect(ray);
                            hit.zip(new_hit).map_or_else(
                                || hit.or(new_hit),
                                |(hit, new_hit)| {
                                    if hit.distance < new_hit.distance {
                                        Some(hit)
                                    } else {
                                        Some(new_hit)
                                    }
                                },
                            )
                        })
                    }

                    fn ray_trace(
                        ray: Ray,
                        objects: &[Object],
                        rng: &mut dyn rand::RngCore,
                        depth: usize,
                    ) -> Vector3<f32> {
                        if depth == 0 {
                            return Vector3::zero();
                        }

                        let hit = get_closest_object(ray, objects);

                        if let Some(hit) = hit {
                            fn random_in_direction(
                                rng: &mut dyn rand::RngCore,
                                direction: Vector3<f32>,
                            ) -> Vector3<f32> {
                                let random = Vector3 {
                                    x: rng.gen::<f32>() * 2.0 - 1.0,
                                    y: rng.gen::<f32>() * 2.0 - 1.0,
                                    z: rng.gen::<f32>() * 2.0 - 1.0,
                                };
                                random * random.dot(direction).signum().into()
                            }

                            let direction = ray.direction.reflect(hit.normal);

                            let mut in_color: Vector3<f32> = Vector3::zero();
                            for _ in 0..SAMPLES_PER_BOUNCE {
                                in_color += ray_trace(
                                    Ray {
                                        origin: hit.position + hit.normal * 0.001.into(),
                                        direction: random_in_direction(rng, direction)
                                            * (1.0 - hit.material.reflectiveness).into()
                                            + direction * hit.material.reflectiveness.into(),
                                    },
                                    objects,
                                    rng,
                                    depth - 1,
                                );
                            }
                            in_color *= (1.0 / SAMPLES_PER_BOUNCE as f32).into();

                            hit.material.emit_color + hit.material.diffuse_color * in_color
                        } else {
                            let t = ray.direction.y * 0.5 + 0.5;
                            let up_color: Vector3<f32> = (1.0, 1.0, 1.0).into();
                            let down_color: Vector3<f32> = (0.5, 0.7, 1.0).into();
                            up_color * (1.0 - t).into() + down_color * t.into()
                        }
                    }

                    let mut rng = rand::thread_rng();

                    let mut color = Vector3::<f32>::zero();
                    for _ in 0..SAMPLES_PER_BOUNCE {
                        let uv = Vector2 {
                            x: (x as f32 + rng.gen::<f32>() * 2.0 - 1.0) / width as f32,
                            y: (y as f32 + rng.gen::<f32>() * 2.0 - 1.0) / height as f32,
                        };
                        let ray = Ray {
                            origin: camera_position,
                            direction: ((camera_right * ((uv.x * 2.0 - 1.0) * aspect).into())
                                + (camera_up * (uv.y * 2.0 - 1.0).into())
                                + camera_forward)
                                .normalized(),
                        };
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
