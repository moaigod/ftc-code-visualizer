use eframe::glow;
use glam::{Mat4, Vec3};

pub struct FieldRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    opaque_vbo: glow::Buffer,
    transparent_vbo: glow::Buffer,
    opaque_vertex_count: i32,
    transparent_vertex_count: i32,
    opaque_source_ptr: usize,
    transparent_sorted: Vec<[f32; 7]>,
}

impl FieldRenderer {
    pub fn new(gl: &glow::Context) -> Self {
        use glow::HasContext as _;
        unsafe {
            let program = gl.create_program().expect("create 3D field shader program");
            let vertex_source = r#"#version 330
                layout(location = 0) in vec3 a_position;
                layout(location = 1) in vec3 a_color;
                layout(location = 2) in float a_alpha;
                uniform mat4 u_view_projection;
                out vec3 v_color;
                out float v_alpha;
                void main() {
                    v_color = a_color;
                    v_alpha = a_alpha;
                    gl_Position = u_view_projection * vec4(a_position, 1.0);
                }"#;
            let fragment_source = r#"#version 330
                in vec3 v_color;
                in float v_alpha;
                out vec4 out_color;
                void main() { out_color = vec4(v_color, v_alpha); }"#;
            let mut shaders = Vec::new();
            for (kind, source) in [(glow::VERTEX_SHADER, vertex_source), (glow::FRAGMENT_SHADER, fragment_source)] {
                let shader = gl.create_shader(kind).expect("create 3D field shader");
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                assert!(gl.get_shader_compile_status(shader), "3D field shader compilation failed: {}", gl.get_shader_info_log(shader));
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            assert!(gl.get_program_link_status(program), "3D field shader link failed: {}", gl.get_program_info_log(program));
            for shader in shaders { gl.detach_shader(program, shader); gl.delete_shader(shader); }
            let vao = gl.create_vertex_array().expect("create 3D field VAO");
            let opaque_vbo = gl.create_buffer().expect("create 3D opaque CAD VBO");
            let transparent_vbo = gl.create_buffer().expect("create 3D transparent CAD VBO");
            gl.bind_vertex_array(Some(vao));
            Self {
                program, vao, opaque_vbo, transparent_vbo,
                opaque_vertex_count: 0, transparent_vertex_count: 0,
                opaque_source_ptr: 0,
                transparent_sorted: Vec::new(),
            }
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.opaque_vbo);
            gl.delete_buffer(self.transparent_vbo);
        }
    }

    pub fn paint(
        &mut self,
        gl: &glow::Context,
        camera_yaw: f32,
        camera_pitch: f32,
        camera_distance: f32,
        aspect_ratio: f32,
        opaque_vertices: Option<&[[f32; 7]]>,
        transparent_vertices: Option<&[[f32; 7]]>,
    ) {
        use glow::HasContext as _;

        let opaque_ptr = opaque_vertices.map_or(0, |v| v.as_ptr() as usize);
        if let Some(vertices) = opaque_vertices {
            if opaque_ptr != self.opaque_source_ptr || vertices.len() as i32 != self.opaque_vertex_count {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.opaque_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(vertices), glow::STATIC_DRAW);
                }
                self.opaque_source_ptr = opaque_ptr;
                self.opaque_vertex_count = vertices.len() as i32;
            }
        }

        let target = Vec3::new(72.0, 72.0, 0.0);
        let cp = camera_pitch.cos();
        let eye = target + Vec3::new(
            camera_distance * cp * camera_yaw.cos(),
            camera_distance * cp * camera_yaw.sin(),
            camera_distance * camera_pitch.sin(),
        );
        let view = Mat4::look_at_rh(eye, target, Vec3::Z);
        let projection = Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect_ratio.max(0.1), 0.1, 1000.0);
        let view_projection = projection * view;

        // Transparent triangles must be drawn back-to-front. STEP assemblies
        // commonly contain overlapping translucent panels, so source order
        // alone can make them appear opaque or incorrectly layered.
        self.transparent_sorted.clear();
        if let Some(vertices) = transparent_vertices {
            let mut triangle_order = vertices.chunks_exact(3).enumerate()
                .map(|(index, triangle)| {
                    let center = Vec3::new(
                        (triangle[0][0] + triangle[1][0] + triangle[2][0]) / 3.0,
                        (triangle[0][1] + triangle[1][1] + triangle[2][1]) / 3.0,
                        (triangle[0][2] + triangle[1][2] + triangle[2][2]) / 3.0,
                    );
                    (center.distance_squared(eye), index)
                })
                .collect::<Vec<_>>();
            triangle_order.sort_by(|a, b| b.0.total_cmp(&a.0));
            self.transparent_sorted.reserve(vertices.len());
            for (_, index) in triangle_order {
                let start = index * 3;
                self.transparent_sorted.extend_from_slice(&vertices[start..start + 3]);
            }
            self.transparent_vertex_count = self.transparent_sorted.len() as i32;
        } else {
            self.transparent_vertex_count = 0;
        }

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));
            if let Some(location) = gl.get_uniform_location(self.program, "u_view_projection") {
                gl.uniform_matrix_4_f32_slice(Some(&location), false, &view_projection.to_cols_array());
            }
            gl.bind_vertex_array(Some(self.vao));

            // Opaque CAD geometry is uploaded once and drawn directly from its GPU buffer.
            if self.opaque_vertex_count > 0 {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.opaque_vbo));
                Self::set_vertex_layout(gl);
                gl.disable(glow::BLEND);
                gl.depth_mask(true);
                gl.draw_arrays(glow::TRIANGLES, 0, self.opaque_vertex_count);
            }

            // Transparent CAD geometry is also cached. Keep depth testing enabled
            // but avoid depth writes so transparent surfaces do not hide one another.
            if self.transparent_vertex_count > 0 {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.transparent_vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&self.transparent_sorted),
                    glow::DYNAMIC_DRAW,
                );
                Self::set_vertex_layout(gl);
                gl.enable(glow::BLEND);
                gl.blend_equation(glow::FUNC_ADD);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.draw_arrays(glow::TRIANGLES, 0, self.transparent_vertex_count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
            }

            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    unsafe fn set_vertex_layout(gl: &glow::Context) {
        use glow::HasContext as _;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 28, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 28, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, 28, 24);
    }
}
