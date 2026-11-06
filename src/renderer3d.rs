use eframe::egui::{self, Rect};
use eframe::{egui_glow, glow};
use glam::{Mat4, Vec3};
use std::sync::Arc;

pub struct FieldRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
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
                out vec3 v_color;
                void main() {
                    v_color = a_color;
                    gl_Position = u_view_projection * vec4(a_position, 1.0);
                }"#;
            let fragment_source = r#"#version 330
                in vec3 v_color;
                out vec4 out_color;
                void main() { out_color = vec4(v_color, 1.0); }"#;
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
            let vbo = gl.create_buffer().expect("create 3D field VBO");
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);
            gl.bind_vertex_array(None);
            Self { program, vao, vbo }
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
        }
    }

    pub fn paint(
        &self,
        gl: &glow::Context,
        camera_yaw: f32,
        camera_pitch: f32,
        camera_distance: f32,
        robot_x: f32,
        robot_y: f32,
        heading_deg: f32,
        red_hives: [(f32, f32); 2],
        blue_hives: [(f32, f32); 2],
        pieces: &[(f32, f32, f32, [f32; 3])],
    ) {
        use glow::HasContext as _;
        let mut mesh = Mesh::default();
        mesh.cuboid([72.0,72.0,-0.7],[72.0,72.0,0.7],[0.18,0.23,0.22]);
        // Field perimeter walls: 144 x 144 inches, six inches high.
        mesh.cuboid([72.0,-0.25,3.0],[72.0,0.5,3.0],[0.30,0.34,0.36]);
        mesh.cuboid([72.0,144.25,3.0],[72.0,0.5,3.0],[0.30,0.34,0.36]);
        mesh.cuboid([-0.25,72.0,3.0],[0.5,72.0,3.0],[0.30,0.34,0.36]);
        mesh.cuboid([144.25,72.0,3.0],[0.5,72.0,3.0],[0.30,0.34,0.36]);
        for i in (0..=144).step_by(12) {
            let c = if i % 24 == 0 { [0.40,0.45,0.43] } else { [0.29,0.34,0.32] };
            mesh.cuboid([i as f32,72.0,0.02],[0.035,72.0,0.025],c);
            mesh.cuboid([72.0,i as f32,0.02],[72.0,0.035,0.025],c);
        }
        // Raised center structure. Its dimensions remain a placeholder pending a field CAD reference.
        for (x,y) in [(62.0,62.0),(82.0,62.0),(82.0,82.0),(62.0,82.0)] {
            mesh.cuboid([x,y,6.5],[0.55,0.55,6.5],[0.72,0.73,0.68]);
        }
        for z in [4.0,9.0,13.0] {
            mesh.cuboid([72.0,62.0,z],[10.0,0.35,0.35],[0.72,0.73,0.68]);
            mesh.cuboid([72.0,82.0,z],[10.0,0.35,0.35],[0.72,0.73,0.68]);
            mesh.cuboid([62.0,72.0,z],[0.35,10.0,0.35],[0.72,0.73,0.68]);
            mesh.cuboid([82.0,72.0,z],[0.35,10.0,0.35],[0.72,0.73,0.68]);
        }
        for (x,y) in [(96.5,1.4),(142.6,96.5),(47.5,142.6),(1.4,47.5)] {
            mesh.cuboid([x,y,0.8],[5.0,5.0,0.8],[0.55,0.42,0.18]);
            mesh.cylinder([x,y,1.8],2.7,0.8,[0.95,0.75,0.12]);
        }
        for (x,y) in red_hives {
            mesh.cuboid([x,y,1.25],[11.0,7.0,1.25],[0.66,0.18,0.20]);
            mesh.cuboid([x,y,2.6],[10.2,6.2,0.12],[0.78,0.35,0.35]);
        }
        for (x,y) in blue_hives {
            mesh.cuboid([x,y,1.25],[11.0,7.0,1.25],[0.16,0.32,0.68]);
            mesh.cuboid([x,y,2.6],[10.2,6.2,0.12],[0.32,0.48,0.82]);
        }
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
        let projection = Mat4::perspective_rh_gl(45.0_f32.to_radians(), 1.5, 0.1, 1000.0);
        let view_projection = projection * view;

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
            gl.bind_buffer(glow::ARRAY_BUFFER,Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER,bytemuck::cast_slice(&mesh.vertices),glow::DYNAMIC_DRAW);
            gl.draw_arrays(glow::TRIANGLES,0,(mesh.vertices.len()) as i32);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }
}

#[derive(Default)]
struct Mesh { vertices: Vec<[f32;6]> }

impl Mesh {
    fn tri(&mut self,a:[f32;3],b:[f32;3],c:[f32;3],color:[f32;3]) {
        for p in [a,b,c] { self.vertices.push([p[0],p[1],p[2],color[0],color[1],color[2]]); }
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
