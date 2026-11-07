use eframe::glow;
use glam::{Mat4, Vec3};

pub struct FieldRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    cad_vbo: glow::Buffer,
    transparent_vbo: glow::Buffer,
    cad_vertex_count: i32,
    transparent_vertex_count: i32,
    cad_source_ptr: usize,
    transparent_source_ptr: usize,
}

impl FieldRenderer {
    pub fn new(gl: &glow::Context) -> Self {
        use glow::HasContext as _;
        unsafe {
            let program = gl.create_program().expect("create 3D field shader program");
            let vertex_source = r#"#version 330
                layout(location = 0) in vec3 a_position;
                layout(location = 1) in vec3 a_color;
                uniform mat4 u_view_projection;
                layout(location = 2) in float a_alpha;
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
            let vbo = gl.create_buffer().expect("create 3D dynamic VBO");
            let cad_vbo = gl.create_buffer().expect("create 3D CAD VBO");
            let transparent_vbo = gl.create_buffer().expect("create 3D transparent CAD VBO");
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 28, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, 28, 24);
            gl.bind_vertex_array(None);
            Self {
                program, vao, vbo, cad_vbo, transparent_vbo,
                cad_vertex_count: 0,
                transparent_vertex_count: 0,
                cad_source_ptr: 0,
                transparent_source_ptr: 0,
            }
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_buffer(self.cad_vbo);
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
        robot_x: f32,
        robot_y: f32,
        heading_deg: f32,
        red_hives: [(f32, f32); 2],
        blue_hives: [(f32, f32); 2],
        pieces: &[(f32, f32, f32, [f32; 3])],
        cad_vertices: Option<&[[f32; 7]]>,
        transparent_vertices: Option<&[[f32; 7]]>,
    ) {
        use glow::HasContext as _;
        let mut mesh = Mesh::default();

        // The CAD field is static. Upload it only when a different STEP scene
        // is loaded, rather than copying millions of vertices into a dynamic
        // buffer on every repaint.
        let cad_source_ptr = cad_vertices.map_or(0, |vertices| vertices.as_ptr() as usize);
        if let Some(vertices) = cad_vertices {
            if cad_source_ptr != self.cad_source_ptr || vertices.len() as i32 != self.cad_vertex_count {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.cad_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(vertices), glow::STATIC_DRAW);
                }
                self.cad_source_ptr = cad_source_ptr;
                self.cad_vertex_count = vertices.len() as i32;
            }
        } else {
            self.cad_source_ptr = 0;
            self.cad_vertex_count = 0;
        }
        let transparent_source_ptr = transparent_vertices.map_or(0, |vertices| vertices.as_ptr() as usize);
        if let Some(vertices) = transparent_vertices {
            if transparent_source_ptr != self.transparent_source_ptr || vertices.len() as i32 != self.transparent_vertex_count {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.transparent_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(vertices), glow::STATIC_DRAW);
                }
                self.transparent_source_ptr = transparent_source_ptr;
                self.transparent_vertex_count = vertices.len() as i32;
            }
        } else {
            self.transparent_source_ptr = 0;
            self.transparent_vertex_count = 0;
        }

        // Flower pollen remains represented by the live game-piece data only;
        // do not create separate decorative flower props at pollen spawn points.
        for (x,y,r,color) in pieces {
            mesh.sphere([*x,*y,*r],[*r,*r,*r],*color);
        }
        let h = heading_deg.to_radians();
        let robot_center = [robot_x, robot_y, 3.0];
        mesh.oriented_box(robot_center,[9.0,9.0,2.5],h,[0.19,0.28,0.36]);
        let intake_x = robot_x + h.sin()*9.0;
        let intake_y = robot_y + h.cos()*9.0;
        mesh.oriented_box([intake_x,intake_y,5.8],[6.0,1.0,0.25],h,[0.94,0.62,0.16]);

        let target = Vec3::new(72.0,72.0,0.0);
        let cp = camera_pitch.cos();
        let eye = target + Vec3::new(
            camera_distance*cp*camera_yaw.cos(),
            camera_distance*cp*camera_yaw.sin(),
            camera_distance*camera_pitch.sin(),
        );
        let view = Mat4::look_at_rh(eye,target,Vec3::Z);
        let projection = Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect_ratio.max(0.1), 0.1, 1000.0);
        let view_projection = projection * view;

        // Transparent triangles are sorted far-to-near for conventional alpha blending.
        let mut sorted_transparent = transparent_vertices.unwrap_or(&[]).chunks_exact(3).map(|tri| {
            let depth = tri.iter().map(|v| {
                let delta = Vec3::new(v[0], v[1], v[2]) - eye;
                delta.length_squared()
            }).sum::<f32>() / 3.0;
            (depth, tri)
        }).collect::<Vec<_>>();
        sorted_transparent.sort_by(|a, b| b.0.total_cmp(&a.0));
        let sorted_transparent = sorted_transparent.into_iter()
            .flat_map(|(_, tri)| tri.iter().copied())
            .collect::<Vec<_>>();

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));
            if let Some(location) = gl.get_uniform_location(self.program,"u_view_projection") {
                gl.uniform_matrix_4_f32_slice(Some(&location),false,&view_projection.to_cols_array());
            }
            gl.bind_vertex_array(Some(self.vao));

            // Draw the cached CAD field without re-uploading it.
            if self.cad_vertex_count > 0 {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.cad_vbo));
                gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 28, 0);
                gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 28, 12);
                gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, 28, 24);
                gl.draw_arrays(glow::TRIANGLES, 0, self.cad_vertex_count);
            }

            // Only the robot and moving game pieces use the per-frame buffer.
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.transparent_vbo));
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 28, 0);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 28, 12);
            gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, 28, 24);
            if !sorted_transparent.is_empty() {
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&sorted_transparent), glow::DYNAMIC_DRAW);
                gl.draw_arrays(glow::TRIANGLES, 0, sorted_transparent.len() as i32);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
            }

            // Dynamic robot and game pieces use the compact opaque vertex format.
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 28, 0);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 28, 12);
            gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, 28, 24);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&mesh.vertices), glow::DYNAMIC_DRAW);
            gl.draw_arrays(glow::TRIANGLES, 0, mesh.vertices.len() as i32);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }
}

#[derive(Default)]
struct Mesh { vertices: Vec<[f32;7]> }

impl Mesh {
    fn tri(&mut self,a:[f32;3],b:[f32;3],c:[f32;3],color:[f32;3]) {
        for p in [a,b,c] { self.vertices.push([p[0],p[1],p[2],color[0],color[1],color[2],1.0]); }
    }
    fn quad(&mut self,a:[f32;3],b:[f32;3],c:[f32;3],d:[f32;3],color:[f32;3]) {
        self.tri(a,b,c,color); self.tri(a,c,d,color);
    }
    fn cuboid(&mut self,c:[f32;3],h:[f32;3],color:[f32;3]) {
        let [x,y,z]=c; let [a,b,d]=h;
        let p=[
            [x-a,y-b,z-d],[x+a,y-b,z-d],[x+a,y+b,z-d],[x-a,y+b,z-d],
            [x-a,y-b,z+d],[x+a,y-b,z+d],[x+a,y+b,z+d],[x-a,y+b,z+d],
        ];
        self.quad(p[0],p[3],p[2],p[1],color);
        self.quad(p[4],p[5],p[6],p[7],color);
        self.quad(p[0],p[1],p[5],p[4],color);
        self.quad(p[1],p[2],p[6],p[5],color);
        self.quad(p[2],p[3],p[7],p[6],color);
        self.quad(p[3],p[0],p[4],p[7],color);
    }
    fn oriented_box(&mut self,c:[f32;3],h:[f32;3],angle:f32,color:[f32;3]) {
        let (s,co)=angle.sin_cos();
        let p=[-1.0_f32,1.0];
        let mut points=Vec::new();
        for z in [-h[2],h[2]] {
            for y in p { for x in p {
                points.push([
                    c[0]+x*h[0]*co+y*h[1]*s,
                    c[1]-x*h[0]*s+y*h[1]*co,
                    c[2]+z,
                ]);
            }}
        }
        let q=[points[0],points[1],points[3],points[2],points[4],points[5],points[7],points[6]];
        self.cuboid_from_points(q,color);
    }
    fn cuboid_from_points(&mut self,p:[[f32;3];8],color:[f32;3]) {
        self.quad(p[0],p[3],p[2],p[1],color); self.quad(p[4],p[5],p[6],p[7],color);
        self.quad(p[0],p[1],p[5],p[4],color); self.quad(p[1],p[2],p[6],p[5],color);
        self.quad(p[2],p[3],p[7],p[6],color); self.quad(p[3],p[0],p[4],p[7],color);
    }
    fn sphere(&mut self,c:[f32;3],r:[f32;3],color:[f32;3]) {
        let lat=8; let lon=12;
        for i in 0..lat {
            let a0=-std::f32::consts::FRAC_PI_2+std::f32::consts::PI*i as f32/lat as f32;
            let a1=-std::f32::consts::FRAC_PI_2+std::f32::consts::PI*(i+1) as f32/lat as f32;
            for j in 0..lon {
                let b0=std::f32::consts::TAU*j as f32/lon as f32;
                let b1=std::f32::consts::TAU*(j+1) as f32/lon as f32;
                let point=|a:f32,b:f32| [c[0]+r[0]*a.cos()*b.cos(),c[1]+r[1]*a.cos()*b.sin(),c[2]+r[2]*a.sin()];
                self.quad(point(a0,b0),point(a0,b1),point(a1,b1),point(a1,b0),color);
            }
        }
    }
    fn beam(&mut self,a:[f32;3],b:[f32;3],half_width:f32,half_depth:f32,color:[f32;3]) {
        let av=Vec3::from_array(a); let bv=Vec3::from_array(b);
        let axis=(bv-av).normalize_or_zero();
        if axis.length_squared()<0.5 { return; }
        let reference=if axis.z.abs()<0.92 { Vec3::Z } else { Vec3::Y };
        let side=axis.cross(reference).normalize()*half_width;
        let up=axis.cross(side).normalize()*half_depth;
        let p=[av-side-up,av+side-up,av+side+up,av-side+up,
               bv-side-up,bv+side-up,bv+side+up,bv-side+up];
        self.cuboid_from_points(p.map(|v|v.to_array()),color);
    }
    fn cylinder(&mut self,c:[f32;3],radius:f32,half_height:f32,color:[f32;3]) {
        let n=16;
        for i in 0..n {
            let a=std::f32::consts::TAU*i as f32/n as f32;
            let b=std::f32::consts::TAU*(i+1) as f32/n as f32;
            let p=|angle:f32,z:f32| [c[0]+radius*angle.cos(),c[1]+radius*angle.sin(),c[2]+z];
            self.quad(p(a,-half_height),p(b,-half_height),p(b,half_height),p(a,half_height),color);
            self.tri([c[0],c[1],c[2]+half_height],p(a,half_height),p(b,half_height),color);
        }
    }
}
