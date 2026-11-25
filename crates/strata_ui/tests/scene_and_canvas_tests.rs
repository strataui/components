//! Integration tests for `strata_ui::canvas::Canvas2D` and `strata_ui::scene3d::{SceneViewport, HudCard}`
//! (migrated cleanly from `strata` core tests when `strata-components` moved to `components/`).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use strata::prelude::*;
use strata_render::compositor::DrawCommand;
use strata_render::orbit_controls::OrbitControls;
use strata_render::rasterizer::FrameBuffer;
use strata_render::scene3d::{Background3D, TexturedMaterial3D};
use strata_render::texture3d::TextureStore;
use strata_style::types::Color;
use strata_ui::canvas::{Canvas2D, Canvas2DContext};
use strata_ui::scene3d::{HudCard, SceneViewport};

#[test]
fn test_canvas2d_component_emits_draw_canvas2d_and_compiles_gpu_and_rasterizer() {
    let draw_cb: Rc<dyn Fn(&mut Canvas2DContext)> = Rc::new(|ctx: &mut Canvas2DContext| {
        ctx.set_fill_style(Color::rgba(255, 0, 0, 255));
        ctx.fill_rect(10.0, 10.0, 60.0, 40.0);
        ctx.set_stroke_style(Color::rgba(0, 255, 0, 255));
        ctx.set_line_width(2.0);
        ctx.stroke_rect(10.0, 10.0, 60.0, 40.0);
        ctx.begin_path();
        ctx.move_to(20.0, 80.0);
        ctx.line_to(160.0, 80.0);
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(120.0, 50.0, 15.0, 0.0, std::f32::consts::TAU, false);
        ctx.fill();
    });

    let view = Canvas2D(Some(200.0), Some(120.0), None, Some(draw_cb));
    let mut runtime = AppRuntime::new(view, 240.0, 160.0, 1.0);

    let has_canvas_cmd = runtime.display_list.commands.iter().any(|cmd| {
        matches!(
            cmd,
            DrawCommand::DrawCanvas2D { commands, .. } if commands.len() == 4
        )
    });
    assert!(
        has_canvas_cmd,
        "<Canvas2D> must emit DrawCommand::DrawCanvas2D with 4 recorded commands"
    );

    runtime.set_gpu_2d_paths_enabled(true);
    runtime.compile_gpu_paths_now();
    assert!(
        !runtime.gpu_path_segments().is_empty(),
        "GpuPathPipeline must compile path segments from DrawCommand::DrawCanvas2D"
    );
    let quad_batch =
        strata_render::GpuBatch::from_display_list(&runtime.display_list, 240.0, 160.0);
    assert!(
        quad_batch.len() >= 2,
        "GpuBatch must compile FillRect and StrokeRect from DrawCommand::DrawCanvas2D"
    );

    let mut fb = FrameBuffer::new(240, 160);
    fb.execute_display_list(&runtime.display_list);
    let px = fb.get_pixel(30, 30);
    assert_eq!(px, Color::rgba(255, 0, 0, 255));
}

#[test]
fn test_scene_viewport_programmatic_scene_textures_and_hud_stacking_order() {
    let mut store = TextureStore::new();
    let tex_pixels = vec![
        16, 196, 248, 255, 16, 196, 248, 255, 16, 196, 248, 255, 16, 196, 248, 255,
    ];
    let tex_id = store
        .insert_rgba8(2, 2, tex_pixels)
        .expect("2x2 RGBA8 texture must insert");

    let quad_geom = Geometry3D::CustomTextured {
        vertices: vec![
            -10.0, -10.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0,
            10.0, -10.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0,
            10.0, 10.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
            -10.0, 10.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    let mut tex_mat = TexturedMaterial3D::new(tex_id);
    tex_mat.tint = Color::rgba(255, 255, 255, 255);
    tex_mat.lit = false;

    let mut scene = Scene3D::new()
        .with_background(Background3D::Solid(Color::rgba(5, 8, 15, 255)))
        .with_textures(Arc::new(store));
    scene.camera = Camera3D::Perspective {
        fov_horizontal_deg: 60.0,
        near: 0.1,
        far: 100.0,
        position: [0.0, 0.0, 5.0],
        target: [0.0, 0.0, 0.0],
    };
    scene.add_mesh(Mesh3D::new(quad_geom, Material3D::Textured(tex_mat)));
    let shared_scene = Arc::new(scene);

    let hud_color_hex =
        "position: relative; z-index: 10; background-color: #ff2040; width: 200px; height: 60px;";
    let view = rsx! {
        <div class="flex flex-row w-full h-full">
            <div class="w-48 h-full bg-slate-900">"Sidebar"</div>
            <div class="flex-1 h-full">
                <SceneViewport scene={&shared_scene} class="w-full h-full overflow-hidden">
                    <div style={hud_color_hex}>
                        <HudCard title="Telemetry HUD">"ALT 1240m"</HudCard>
                    </div>
                </SceneViewport>
            </div>
            <strata_dom::Portal>
                <div style="position: fixed; left: 260px; top: 120px; width: 120px; height: 50px; background-color: #20ff60;">
                    "Portal Menu"
                </div>
            </strata_dom::Portal>
        </div>
    };

    let mut runtime = AppRuntime::new(view, 800.0, 600.0, 1.0);
    runtime.tick_frame();

    let vp_indices: Vec<usize> = runtime
        .display_list
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, cmd)| matches!(cmd, DrawCommand::DrawViewport3D { .. }).then_some(i))
        .collect();
    assert_eq!(vp_indices.len(), 1);
    let vp_idx = vp_indices[0];

    let hud_idx = runtime
        .display_list
        .commands
        .iter()
        .position(|cmd| {
            matches!(
                cmd,
                DrawCommand::DrawRect { color, .. }
                    if color.r == 0xff && color.g == 0x20 && color.b == 0x40
            )
        })
        .expect("Expected z-10 HUD #ff2040 DrawRect in display list");
    let portal_idx = runtime
        .display_list
        .commands
        .iter()
        .position(|cmd| {
            matches!(
                cmd,
                DrawCommand::DrawRect { color, .. }
                    if color.r == 0x20 && color.g == 0xff && color.b == 0x60
            )
        })
        .expect("Expected Portal #20ff60 DrawRect in display list");

    assert!(vp_idx < hud_idx);
    assert!(hud_idx < portal_idx);

    let mut fb = FrameBuffer::new(800, 600);
    fb.execute_display_list(&runtime.display_list);

    let pixel_at = |fb: &FrameBuffer, x: usize, y: usize| -> [u8; 4] {
        let off = (y * fb.width + x) * 4;
        [
            fb.pixels[off],
            fb.pixels[off + 1],
            fb.pixels[off + 2],
            fb.pixels[off + 3],
        ]
    };

    let scene_px = pixel_at(&fb, 500, 400);
    assert!(
        (scene_px[0] as i32 - 16).abs() <= 2
            && (scene_px[1] as i32 - 196).abs() <= 2
            && (scene_px[2] as i32 - 248).abs() <= 2,
        "Expected satellite texture RGB ~(16, 196, 248), got {:?}",
        scene_px
    );

    let hud_px = pixel_at(&fb, 385, 30);
    assert!(hud_px[0] > hud_px[1] && hud_px[0] > hud_px[2] && hud_px[0] > 40);

    let portal_px = pixel_at(&fb, 370, 160);
    assert_eq!([portal_px[0], portal_px[1], portal_px[2]], [0x20, 0xff, 0x60]);
}

#[test]
fn test_scene_viewport_on_camera_change_pointer_and_wheel_interaction() {
    let mut scene = Scene3D::new();
    scene.camera = Camera3D::Perspective {
        fov_horizontal_deg: 60.0,
        near: 0.1,
        far: 1000.0,
        position: [0.0, 0.0, 20.0],
        target: [0.0, 0.0, 0.0],
    };

    let observed: Rc<RefCell<Vec<(f32, f32, f32)>>> = Rc::new(RefCell::new(Vec::new()));
    let observed_cb = Rc::clone(&observed);

    let view = rsx! {
        <SceneViewport
            scene={scene}
            width={600.0}
            height={400.0}
            on_camera_change={move |ctrl: &OrbitControls| {
                observed_cb.borrow_mut().push((ctrl.theta, ctrl.phi, ctrl.radius));
            }}
        />
    };

    let mut runtime = AppRuntime::new(view, 600.0, 400.0, 1.0);
    runtime.tick_frame();

    runtime.dispatch_pointer_down(300.0, 200.0);
    runtime.dispatch_pointer_move(360.0, 230.0);
    runtime.dispatch_pointer_up(360.0, 230.0);

    assert_eq!(observed.borrow().len(), 1);
    let (theta_after_drag, _phi_after_drag, radius_after_drag) = observed.borrow()[0];
    assert!(theta_after_drag.abs() > 0.05);
    assert!((radius_after_drag - 20.0).abs() < 1e-3);

    let wheel_handled = runtime.dispatch_scroll(300.0, 200.0, 0.0, 5.0);
    assert!(wheel_handled);
    assert_eq!(observed.borrow().len(), 2);
    let (_theta_after_zoom, _phi_after_zoom, radius_after_zoom) = observed.borrow()[1];
    assert!((radius_after_zoom - 20.0).abs() > 0.1);
}
