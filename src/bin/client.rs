use std::rc::Rc;
use std::cell::RefCell;

extern crate failure;
#[allow(unused_imports)]
use failure::Fail;

extern crate time;
extern crate glfw;
use glfw::{Context, Key, Action};
extern crate gl;

extern crate tcge;
use tcge::resources::Resources;
use tcge::blocks::universe;
use tcge::client::cmd_opts;
use tcge::client::render;
use tcge::client::geometry;
use tcge::client::freecam;
use tcge::gameloop;
use std::sync::mpsc::Receiver;

fn main() {
	// TODO: Attempt to merge the two separate error-printers into one...
	
	let options = match cmd_opts::parse() {
		Err(e) => {
			use std::fmt::Write;
			let mut result = String::new();
			
			for (i, cause) in e.causes().collect::<Vec<_>>().into_iter().enumerate() {
				if i > 0 {
					let _ = write!(&mut result, "   Caused by: ");
				}
				let _ = write!(&mut result, "{}", cause);
				if let Some(backtrace) = cause.backtrace() {
					let backtrace_str = format!("{}", backtrace);
					if !backtrace_str.is_empty() {
						let _ = writeln!(&mut result, " This happened at {}", backtrace);
					} else {
						let _ = writeln!(&mut result);
					}
				} else {
					let _ = writeln!(&mut result);
				}
			}
			
			println!("{}\n", result);
			panic!("Failed to parse command-line arguments! Exiting...");
		}
		Ok(o) => o
	};
	
	if let Err(e) = run(options) {
		use std::fmt::Write;
		let mut result = String::new();
		
		for (i, cause) in e.causes().collect::<Vec<_>>().into_iter().enumerate() {
			if i > 0 {
				let _ = write!(&mut result, "   Caused by: ");
			}
			let _ = write!(&mut result, "{}", cause);
			if let Some(backtrace) = cause.backtrace() {
				let backtrace_str = format!("{}", backtrace);
				if !backtrace_str.is_empty() {
					let _ = writeln!(&mut result, " This happened at {}", backtrace);
				} else {
					let _ = writeln!(&mut result);
				}
			} else {
				let _ = writeln!(&mut result);
			}
		}
		
		println!("{}", result);
	}
	
	println!("\nGoodbye!\n");
}

fn new_window(
	glfw: &mut glfw::Glfw,
	opts: &cmd_opts::CmdOptions
) -> (glfw::Window, Receiver<(f64, glfw::WindowEvent)>) {
	
	glfw.window_hint(glfw::WindowHint::ContextVersion(3,2));
	glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
	
	#[cfg(target_os = "macos")]
		glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
	glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));
	glfw.window_hint(glfw::WindowHint::Samples(Some(opts.gl_multisamples)));
	
	// ------------------------------------------
	let (mut window, events) = glfw.create_window(
		1024, 768, "Talecraft",
		glfw::WindowMode::Windowed
	).expect("Failed to create GLFW window.");
	
	window.make_current();
	window.set_key_polling(true);
	window.set_cursor_pos_polling(true);
	window.set_cursor_mode(glfw::CursorMode::Disabled);
	window.set_framebuffer_size_polling(true);
	window.set_size_limits(
		320, 225,
		glfw::ffi::DONT_CARE as u32,
		glfw::ffi::DONT_CARE as u32
	);
	
	// ------------------------------------------
	gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);
	
	return (window, events);
}

fn run(opts: cmd_opts::CmdOptions) -> Result<(), failure::Error> {
	// ------------------------------------------
	let res = Resources::from_exe_path()?;
	
	// ------------------------------------------
	let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;
	let (mut window, events) = new_window(&mut glfw, &opts);
	
	/*
	unsafe {
		let depth_bits = glfw::ffi::glfwGetWindowAttrib(window.window_ptr(), glfw::ffi::DEPTH_BITS);
		let depth_bits = gl::GetFramebufferAttachmentParameteriv(gl::FRAMEBUFFER, gl::GL_DEPTH_ATTACHMENT);
		println!("Available depth bits: {}", depth_bits);
	}
	*/
	
	// ------------------------------------------
	let shader_grid = render::materials::ShaderGrid::new(&res)?;
	let shader_random = render::materials::ShaderRandom::new(&res)?;
	let shader_solid_color = render::materials::ShaderSolidColor::new(&res)?;
	
	let mut ascii_renderer = render::ascii_text::AsciiTextRenderer::load(&res)?;
	
	// ------------------------------------------
	let mut render_state_gui = GuiRenderState {
		width: 0, height: 0,
		ascii_renderer,
	};
	
	// ------------------------------------------
	let mut render_state = RenderState {
		frame_id: 0,
		shader_grid,
		shader_random,
		shader_solid_color
	};
	
	let mut cursor = Cursor {pos_x: 0.0, pos_y: 0.0, mov_x: 0.0, mov_y: 0.0};
	
	let block_universe = universe::define_universe();
	
	let scene = Rc::new(RefCell::new(Option::Some(Scene {
		camera: freecam::Camera::new(),
		meshes: vec![
			geometry::geometry_test(),
			geometry::geometry_cube(1.0),
			// geometry::geometry_cube(-512.0),
		],
		mesh_planequad: geometry::geometry_planequad(1024.0),
		block_universe: block_universe
	})));
	
	// ------------------------------------------
	let mut gls = gameloop::new_gameloop(30);
	
	while !window.should_close() {
		process_events(
			&mut window,
			&events,
			&mut cursor,
			&mut *scene.borrow_mut()
		);
		
		let window_size = window.get_framebuffer_size();
		let mut reset_render_state = false;
		
		gameloop::gameloop_next(&mut gls,
			|| {glfw.get_time()},
			
			|_now:f64| {
				scene.borrow_mut().as_mut().map(|mut_scene| {
					mut_scene.camera.update_movement(&window);
				});
				
				reset_render_state = true;
			},
			
			|now: f64, interpolation: f32| {
				(&mut render_state).begin();
				scene.borrow().as_ref().map(|scene| {
					render(
						&render_state,
						&scene,
						&scene.camera,
						window_size,
						now,
						interpolation
					);
				});
				(&mut render_state).end();
				
				let (w, h) = window.get_framebuffer_size();
				render_state_gui.width = w;
				render_state_gui.height = h;
				render_gui(&mut render_state_gui);
			}
		);
		
		if reset_render_state {
			(&mut render_state).reset();
		}
		
		window.swap_buffers();
		glfw.poll_events();
	}
	
	Ok(())
}

fn process_events(
	window: &mut glfw::Window,
	events: &std::sync::mpsc::Receiver<(f64, glfw::WindowEvent)>,
	cursor: &mut Cursor,
	opt_scene: &mut Option<Scene>
) {
	for(_, event) in glfw::flush_messages(events) {
		match event {
			glfw::WindowEvent::FramebufferSize(width, height) => {
				unsafe {gl::Viewport(0, 0, width, height)}
			},
			
			glfw::WindowEvent::Key(Key::M, _, Action::Press, _) => {
				if window.get_cursor_mode() == glfw::CursorMode::Disabled {
					window.set_cursor_mode(glfw::CursorMode::Normal);
				} else {
					window.set_cursor_mode(glfw::CursorMode::Disabled);
				}
				
				opt_scene.as_mut()
					.map(|mut_scene| &mut mut_scene.camera)
					.map( |mut_camera| {
						mut_camera.active = window.get_cursor_mode() == glfw::CursorMode::Disabled;
					});
			},
			
			glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
				window.set_should_close(true)
			},
			
			glfw::WindowEvent::CursorPos(x, y) => {
				cursor.update(x, y);
				opt_scene.as_mut()
					.map(|mut_scene| &mut mut_scene.camera)
					.map( |mut_camera| {
						mut_camera.update_rotation(
							cursor.mov_x,
							cursor.mov_y
						);
					});
			},
			_ => ()
		}
	}
}

struct Scene {
	camera: freecam::Camera,
	meshes: Vec<geometry::SimpleVao>,
	mesh_planequad: geometry::SimpleVao,
	block_universe: universe::BlockUniverse,
}

struct Cursor {
	pos_x: f32,
	pos_y: f32,
	mov_x: f32,
	mov_y: f32,
}

impl Cursor {
	fn update(&mut self, x: f64, y: f64) {
		self.mov_x = (x as f32) - self.pos_x;
		self.mov_y = (y as f32) - self.pos_y;
		self.pos_x = x as f32;
		self.pos_y = y as f32;
	}
}

struct RenderState {
	frame_id: i64,
	shader_grid: render::materials::ShaderGrid,
	shader_random: render::materials::ShaderRandom,
	shader_solid_color: render::materials::ShaderSolidColor,
}

impl RenderState {
	fn begin(&mut self) {
		self.frame_id = self.frame_id + 1;
	}
	fn end(&mut self) {}
	
	fn reset(&mut self) {
		self.frame_id = 0;
	}
}

fn render(render_state: &RenderState, scene: &Scene, camera: &freecam::Camera, size: (i32, i32), now: f64, _interpolation:f32) {
	unsafe {
		gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
		gl::Enable(gl::DEPTH_TEST);
		gl::CullFace(gl::FRONT);
		gl::Enable(gl::CULL_FACE);
	}
	
	/*
	println!("Render Frame [ id: {}, size: {} x {}, time: {}, delta: {}]",
		render_state.frame_id, size.0, size.1, now, _interpolation
	);
	*/
	
	let camera_transform = camera.transform(size, _interpolation, true);
	
	/*
	let shader_solid_color = &render_state.shader_solid_color;
	let grid_color = cgmath::Vector4 {x: 1.0, y: 1.0, z: 1.0, w: 1.0};
	shader_solid_color.shader_program.set_used();
	shader_solid_color.shader_program.uniform_matrix4(shader_solid_color.uniform_matrix, camera_transform);
	shader_solid_color.shader_program.uniform_vector4(shader_solid_color.uniform_color, grid_color);
	scene.mesh_grid.draw(gl::LINES);
	*/
	
	unsafe {
		gl::Enable(gl::BLEND);
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
		gl::Disable(gl::DEPTH_TEST);
	}
	let shader_grid = &render_state.shader_grid;
	shader_grid.shader_program.set_used();
	shader_grid.shader_program.uniform_matrix4(shader_grid.uniform_matrix, camera_transform);
	scene.mesh_planequad.draw(gl::TRIANGLES);
	unsafe {
		gl::Enable(gl::DEPTH_TEST);
		gl::Disable(gl::BLEND);
	}
	
	let shader_random = &render_state.shader_random;
	shader_random.shader_program.set_used();
	shader_random.shader_program.uniform_matrix4(shader_random.uniform_matrix, camera_transform);
	shader_random.shader_program.uniform_scalar(shader_random.uniform_time, now as f32);
	
	for mesh in scene.meshes.iter() {
		mesh.draw(gl::TRIANGLES);
	}
}

struct GuiRenderState {
	width: i32, height: i32,
	ascii_renderer: render::ascii_text::AsciiTextRenderer,
}

fn render_gui(render_state_gui: &mut GuiRenderState) {
	
	unsafe {
		gl::Flush();
		gl::Enable(gl::BLEND);
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
		gl::Disable(gl::DEPTH_TEST);
	}
	
	let projection = cgmath::ortho(0.0,
		render_state_gui.width as f32,
		render_state_gui.height as f32,
		0.0,
		-1.0,1.0
	);
	
	render_state_gui.ascii_renderer.transform = projection;
	render_state_gui.ascii_renderer.draw_text("Hello, World!".to_string(), 0.0, 0.0);
	
}