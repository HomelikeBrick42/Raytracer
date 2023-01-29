use rayon::{
    prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use thallium::{
    math::{Matrix4x4, One, Vector2, Vector3, Zero},
    platform::{Surface, SurfaceEvent},
    renderer::{Pixels, PrimitiveType, RendererAPI, VertexBufferElement},
    scene::Camera,
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

    renderer.get_surface_mut().show();
    'main_loop: loop {
        for event in renderer.get_surface_mut().events() {
            match event {
                SurfaceEvent::Close => break 'main_loop,
                SurfaceEvent::Resize(Vector2 {
                    x: width,
                    y: height,
                }) => {
                    pixels = vec![Vector3::zero(); width * height];
                }
                _ => {}
            }
        }

        let size @ Vector2 {
            x: width,
            y: height,
        } = renderer.get_surface_mut().get_size();

        // Ray trace stuff
        pixels
            .par_chunks_mut(width)
            .enumerate()
            .flat_map(|(y, row)| {
                row.par_iter_mut()
                    .enumerate()
                    .map(move |(x, pixel)| (x, y, pixel))
            })
            .for_each(|(x, y, pixel)| {
                let uv = Vector2 {
                    x: x as f32 / width as f32,
                    y: y as f32 / height as f32,
                };

                *pixel = Vector3 {
                    x: uv.x,
                    y: uv.y,
                    z: 0.0,
                };
            });

        // Render to window
        renderer.clear(Vector3::zero());
        {
            renderer
                .get_texture_mut(texture)
                .unwrap()
                .set_pixels(size, Pixels::RGBF(&pixels));
            let mut draw_context = renderer.drawing_context(Camera::default(), false);
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
    }
    renderer.get_surface_mut().hide();
}
