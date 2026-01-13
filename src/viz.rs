use std::{collections::HashMap, path::PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    color::palettes::basic,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
    window::WindowPlugin,
};
use bevy_image_export::{ImageExport, ImageExportPlugin, ImageExportSettings, ImageExportSource};
use hexx::mesh::{MeshInfo, PlaneMeshBuilder};
use hexx::*;

/// Input resource: your sparse colored tiles.
#[derive(Resource)]
struct TileColors(HashMap<Hex, Color>);

/// Rendering config (output + sizing).
#[derive(Resource)]
struct RenderConfig {
    output_path: PathBuf,
    image_width: u32,
    image_height: u32,
    hex_size: f32, // hexx layout "regular size" (world units)
    margin_world: f32,
}

/// Hold onto the render target handle.
#[derive(Resource)]
struct TargetImage(Handle<Image>);

fn mk_hex(col: i32, row: i32) -> Hex {
    Hex::from_doubled_coordinates([col, row], DoubledHexMode::DoubledHeight)
}

pub fn render() {
    // Example data (replace this with your own HashMap<Hex, Color>).
    let tiles: HashMap<Hex, Color> = [
        (mk_hex(0, 0), Color::WHITE),
        (mk_hex(0, 2), Color::WHITE),
        (mk_hex(0, 4), Color::from(basic::BLUE)),
        (mk_hex(0, 6), Color::from(basic::BLUE)),
        (mk_hex(0, 8), Color::BLACK),
        (mk_hex(1, 1), Color::WHITE),
        (mk_hex(1, 3), Color::WHITE),
        (mk_hex(1, 5), Color::WHITE),
        (mk_hex(1, 7), Color::WHITE),
    ]
    .into();

    let cfg = RenderConfig {
        output_path: "out.png".into(),
        image_width: 400,
        image_height: 300,
        hex_size: 32.0,
        margin_world: 16.0,
    };

    let export_plugin = ImageExportPlugin::default();
    let export_threads = export_plugin.threads.clone();

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (cfg.image_width, cfg.image_height).into(),
                    ..default()
                }),
                ..default()
            }),
            export_plugin,
        ))
        .insert_resource(TileColors(tiles))
        .insert_resource(cfg)
        .add_systems(Startup, setup)
        .add_systems(Update, exit)
        .run();

    export_threads.finish();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut export_sources: ResMut<Assets<ImageExportSource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    tiles: Res<TileColors>,
    cfg: Res<RenderConfig>,
) {
    let layout = HexLayout::flat()
        .with_hex_size(cfg.hex_size)
        .with_origin(Vec2::ZERO);

    let (min_x, max_x, min_y, max_y) =
        bounds_from_hex_corners(&layout, tiles.0.keys(), cfg.margin_world);

    println!("{:?}", (min_x, max_x, min_y, max_y));

    // Create an output texture.
    let output_texture_handle = {
        let size = Extent3d {
            width: cfg.image_width,
            height: cfg.image_height,
            ..default()
        };
        let mut export_texture = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
            ..default()
        };
        export_texture.resize(size);

        images.add(export_texture)
    };

    // Camera: orthographic, looking straight down at the XZ plane.
    // hexx PlaneMeshBuilder generates vertices on XZ (Y up) by default. :contentReference[oaicite:3]{index=3}
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;

    let world_w = (max_x - min_x).max(0.001);
    let world_h = (max_y - min_y).max(0.001);

    let aspect = cfg.image_width as f32 / cfg.image_height as f32;

    // Fit bounds while preserving output aspect:
    let mut view_w = world_w;
    let mut view_h = world_h;
    if view_w / view_h > aspect {
        // too wide -> expand height
        view_h = view_w / aspect;
    } else {
        // too tall -> expand width
        view_w = view_h * aspect;
    }

    let projection = OrthographicProjection {
        // Fixed vertical+horizontal span in world units
        scaling_mode: bevy::camera::ScalingMode::Fixed {
            width: view_w,
            height: view_h,
        },
        near: -1000.0,
        far: 1000.0,
        ..OrthographicProjection::default_2d()
    };

    println!("{projection:?}");

    for target in [
        RenderTarget::Image(output_texture_handle.clone().into()),
        RenderTarget::Window(bevy::window::WindowRef::Primary),
    ] {
        commands.spawn((
            Camera2d::default(),
            Camera {
                target,
                ..default()
            },
            Projection::Orthographic(projection.clone()),
            Transform::from_xyz(center_x, center_y, 0.),
            //     .looking_at(Vec3::new(center_x, center_y, 0.), Vec3::Z),
        ));
    }

    // Spawn the ImageExport component to initiate the export of the output texture.
    commands.spawn((
        ImageExport(export_sources.add(output_texture_handle)),
        ImageExportSettings {
            // Frames will be saved to "./out/[#####].png".
            output_dir: "out".into(),
            // Choose "exr" for HDR renders.
            extension: "png".into(),
        },
    ));

    // Reuse ONE hex mesh; place each tile via Transform translation.
    let hexx_mesh_info = PlaneMeshBuilder::new(&layout)
        .facing(Vec3::Z)
        .center_aligned()
        .build();
    info!("{hexx_mesh_info:?}");
    let mesh_handle = meshes.add(mesh_from_hexx_mesh_info(hexx_mesh_info));

    // Spawn each colored tile
    for (h, color) in tiles.0.iter() {
        let pos = layout.hex_to_world_pos(*h);
        commands.spawn((
            Mesh2d(mesh_handle.clone()),
            MeshMaterial2d(materials.add(*color)),
            Transform::from_xyz(pos.x, pos.y, 0.),
        ));
    }
}

/// Convert hexx::mesh::MeshInfo to bevy::render::mesh::Mesh.
/// (hexx provides raw vertex/normals/uv/index buffers.) :contentReference[oaicite:6]{index=6}
fn mesh_from_hexx_mesh_info(info: MeshInfo) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, info.vertices)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, info.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, info.uvs)
    .with_inserted_indices(Indices::U16(info.indices))
}

fn bounds_from_hex_corners<'a>(
    layout: &HexLayout,
    hexes: impl Iterator<Item = &'a Hex>,
    margin: f32,
) -> (f32, f32, f32, f32) {
    let mut any = false;

    let mut min_x = 0.0f32;
    let mut max_x = 0.0f32;
    let mut min_y = 0.0f32;
    let mut max_y = 0.0f32;

    for h in hexes {
        let corners = layout.hex_corners(*h); // :contentReference[oaicite:7]{index=7}
        for c in corners {
            if !any {
                any = true;
                min_x = c.x;
                max_x = c.x;
                min_y = c.y;
                max_y = c.y;
            } else {
                min_x = min_x.min(c.x);
                max_x = max_x.max(c.x);
                min_y = min_y.min(c.y);
                max_y = max_y.max(c.y);
            }
        }
    }

    // Handle empty input gracefully: render a blank image.
    if !any {
        return (-1.0, 1.0, -1.0, 1.0);
    }

    (
        min_x - margin,
        max_x + margin,
        min_y - margin,
        max_y + margin,
    )
}

fn exit(mut exit: MessageWriter<AppExit>, mouse: Res<ButtonInput<MouseButton>>) {
    if mouse.just_pressed(MouseButton::Left) {
        exit.write(AppExit::Success);
    }
}
