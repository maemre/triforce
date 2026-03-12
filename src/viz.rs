#![allow(clippy::too_many_arguments)]

use std::{thread, time::Duration};

use ahash::{HashMap, HashSet};

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    color::palettes::{basic, css},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::{
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::screenshot::Capturing,
    },
    window::WindowPlugin,
};
use bevy_image_export::{ImageExport, ImageExportPlugin, ImageExportSettings, ImageExportSource};
use hexx::mesh::{MeshInfo, PlaneMeshBuilder};
use hexx::*;
use pipelines_ready::*;

/// Rendering config (output + sizing).
#[derive(Resource)]
struct RenderConfig {
    output_dir: String,
    image_width: u32,
    image_height: u32,
    hex_size: f32,
    margin_world: f32,
    // frames to wait at the end
    frames_left_to_render: usize,
    // frames to wait between changing tiles
    frames_to_wait: usize,
    // shapes rendered so far
    shapes_rendered: usize,
}

const FRAMES_TO_WAIT_AFTER_LOADING: u8 = 5;
const FRAMES_TO_RENDER: usize = 5;
const BG_COLOR: Color = Color::WHITE;
const PADDING: f32 = 20.;

const COLORS: [Color; 18] = [
    Color::Srgba(basic::BLACK),
    Color::Srgba(basic::AQUA),
    Color::Srgba(basic::BLUE),
    Color::Srgba(basic::FUCHSIA),
    Color::Srgba(basic::GRAY),
    Color::Srgba(basic::GREEN),
    Color::Srgba(basic::LIME),
    Color::Srgba(basic::NAVY),
    Color::Srgba(css::GOLD),
    Color::Srgba(basic::PURPLE),
    Color::Srgba(basic::RED),
    Color::Srgba(basic::SILVER),
    Color::Srgba(css::ORANGE),
    Color::Srgba(basic::YELLOW),
    Color::Srgba(css::PINK),
    Color::Srgba(css::LIGHT_BLUE),
    Color::Srgba(basic::MAROON),
    Color::Srgba(basic::TEAL),
];

#[derive(Resource, Default, PartialEq, Eq)]
enum LoadingStatus {
    // Loading assets and pipelines
    #[default]
    Loading,
    // Counting down buffer frames
    Countdown(u8),
    // Loaded
    Loaded,
}

/// The hexagon drawing data
#[derive(Resource)]
struct HexData {
    // the hexagons to be rendered
    map: HashMap<Hex, Entity>,
    // the label entity
    label: Entity,
}

pub fn mk_hex(col: i32, row: i32) -> Hex {
    Hex::from_doubled_coordinates([col, row], DoubledHexMode::DoubledHeight)
}

const fn resolve_color(color: crate::Color) -> Color {
    COLORS[color.0.get() as usize % COLORS.len()]
}

/// An image to render
#[derive(Resource)]
pub struct RenderData {
    pub tilings: Vec<(String, HashMap<Hex, crate::Color>)>,
}

/// To check whether all assets are loaded
#[derive(Resource)]
struct AssetsToLoad(Vec<UntypedHandle>);

pub fn render(data: RenderData, output_dir: String) {
    let cfg = RenderConfig {
        output_dir,
        image_width: 200,
        image_height: 200,
        hex_size: 32.0,
        margin_world: 16.0,
        frames_left_to_render: FRAMES_TO_RENDER,
        frames_to_wait: 1,
        shapes_rendered: 0,
    };

    let export_plugin = ImageExportPlugin::default();
    let export_threads = export_plugin.threads.clone();

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (cfg.image_width, cfg.image_height).into(),
                    visible: false,
                    ..default()
                }),
                ..default()
            }),
            export_plugin,
        ))
        .add_plugins(PipelinesReadyPlugin)
        .insert_resource(cfg)
        .insert_resource(data)
        .insert_resource(InitialWait(true))
        .insert_resource(LoadingStatus::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                check_loading_status,
                render_next_shape,
                exit_after_all_frames,
            ),
        )
        .run();

    export_threads.finish();
}

#[derive(Resource)]
struct InitialWait(bool);

fn check_loading_status(
    mut loading: ResMut<LoadingStatus>,
    pipelines_ready: Res<PipelinesReady>,
    asset_server: Res<AssetServer>,
    assets_to_load: Res<AssetsToLoad>,
    mut wait: ResMut<InitialWait>,
) {
    use LoadingStatus::*;
    match *loading {
        Loading => {
            let all_assets_loaded = || {
                assets_to_load
                    .0
                    .iter()
                    .all(|asset| asset_server.is_loaded_with_dependencies(asset))
            };

            if pipelines_ready.0 && all_assets_loaded() {
                *loading = Countdown(FRAMES_TO_WAIT_AFTER_LOADING);
            }
        }
        Countdown(0) => *loading = Loaded,
        Countdown(n) => *loading = Countdown(n - 1),
        Loaded => {}
    }

    if *loading == LoadingStatus::Loaded {
        if wait.0 {
            thread::sleep(Duration::from_millis(20));
            wait.0 = false;
        }
        return;
    }

    if pipelines_ready.0 {
        *loading = LoadingStatus::Loaded;
    }
}

fn render_next_shape(
    mut commands: Commands,
    mut data: ResMut<RenderData>,
    mut cfg: ResMut<RenderConfig>,
    hex_data: Res<HexData>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    loading: Res<LoadingStatus>,
    pipelines_ready: Res<PipelinesReady>,
) {
    if *loading != LoadingStatus::Loaded || !pipelines_ready.0 {
        return;
    }

    if cfg.frames_to_wait != 0 {
        cfg.frames_to_wait -= 1;
        return;
    }

    // Spawn each colored tile
    if let Some((label, tiles)) = data.tilings.pop() {
        commands.entity(hex_data.label).insert(Text2d(label));

        assert!(!tiles.is_empty());

        for (hex, entity) in hex_data.map.iter() {
            let color = tiles.get(hex).cloned().map_or(Color::BLACK, resolve_color);
            commands
                .entity(*entity)
                .insert(MeshMaterial2d(materials.add(color)));
        }
        cfg.frames_to_wait = 1;
        cfg.shapes_rendered += 1;
    }
}

fn create_all_tiles(
    commands: &mut Commands,
    data: &RenderData,
    hex_mesh: Handle<Mesh>,
    layout: &HexLayout,
    mut materials: ResMut<Assets<ColorMaterial>>,
) -> HashMap<Hex, Entity> {
    // Spawn each colored tile
    let tiles = data
        .tilings
        .iter()
        .flat_map(|tiles| tiles.1.keys().cloned())
        .collect::<HashSet<_>>();

    tiles
        .into_iter()
        .map(|hex| {
            let pos = layout.hex_to_world_pos(hex);
            let entity = commands
                .spawn((
                    Mesh2d(hex_mesh.clone()),
                    MeshMaterial2d(materials.add(BG_COLOR)),
                    Transform::from_xyz(pos.x, -pos.y, 0.),
                ))
                .id();
            (hex, entity)
        })
        .collect::<HashMap<_, _>>()
}

fn create_label(
    commands: &mut Commands,
    pos: Vec2,
    asset_server: &AssetServer,
    assets_to_load: &mut AssetsToLoad,
) -> Entity {
    let font = asset_server.load("fonts/FiraSans-Bold.otf");
    assets_to_load.0.push(font.clone().untyped());
    let text_font = TextFont {
        font: font.clone(),
        font_size: 50.0,
        ..default()
    };
    let text_justification = Justify::Center;
    commands
        .spawn((
            Text2d::new(""),
            text_font.clone(),
            TextLayout::new_with_justify(text_justification),
            TextBackgroundColor(Color::BLACK.with_alpha(0.2)),
            TextColor(Color::from(basic::BLUE)),
            Transform::from_translation(pos.extend(0.)),
        ))
        .id()
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut export_sources: ResMut<Assets<ImageExportSource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    cfg: Res<RenderConfig>,
    shapes: Res<RenderData>,
    asset_server: Res<AssetServer>,
) {
    let layout = HexLayout::flat()
        .with_hex_size(cfg.hex_size)
        .with_origin(Vec2::ZERO);

    let (min_x, max_x, min_y, max_y) = shapes
        .tilings
        .iter()
        .map(|tiles| bounds_from_hex_corners(&layout, tiles.1.keys(), cfg.margin_world))
        .fold((0f32, 0f32, 0f32, 0f32), |coord1, coord2| {
            (
                coord1.0.min(coord2.0),
                coord1.1.max(coord2.1),
                coord1.2.min(coord2.2),
                coord1.3.max(coord2.3),
            )
        });

    let (min_x, max_x, min_y, max_y) = (
        min_x - PADDING,
        max_x + PADDING,
        min_y - PADDING,
        max_y + PADDING,
    );

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

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;

    let world_w = (max_x - min_x).max(0.001);
    let world_h = (max_y - min_y).max(0.001);

    let aspect = cfg.image_width as f32 / cfg.image_height as f32;

    let mut view_w = world_w;
    let mut view_h = world_h;
    if view_w / view_h > aspect {
        view_h = view_w / aspect;
    } else {
        view_w = view_h * aspect;
    }

    let projection = OrthographicProjection {
        scaling_mode: bevy::camera::ScalingMode::Fixed {
            width: view_w,
            height: view_h,
        },
        ..OrthographicProjection::default_2d()
    };

    println!("{projection:?}");

    // Reuse one hex mesh; place each tile via Transform translation.
    let hexx_mesh_info = PlaneMeshBuilder::new(&layout)
        .facing(Vec3::Z)
        .with_scale(Vec3::splat(0.9))
        .center_aligned()
        .build();
    info!("{hexx_mesh_info:?}");
    let hex_mesh = meshes.add(mesh_from_hexx_mesh_info(hexx_mesh_info));

    let map = create_all_tiles(&mut commands, &shapes, hex_mesh.clone(), &layout, materials);
    let mut assets_to_load = AssetsToLoad(vec![]);
    warn!("{min_x}, {min_y}");
    let label = create_label(
        &mut commands,
        Vec2::new(cfg.image_width as f32 * 2.0 - 100.0, 0.0),
        &asset_server,
        &mut assets_to_load,
    );

    commands.insert_resource(HexData { map, label });
    commands.insert_resource(layout);
    commands.insert_resource(assets_to_load);

    {
        let target = RenderTarget::Image(output_texture_handle.clone().into());
        commands.spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::WHITE),
                ..default()
            },
            Projection::Orthographic(projection.clone()),
            Transform::from_xyz(center_x, center_y, 0.),
            target,
        ));
    }

    commands.spawn((
        ImageExport(export_sources.add(output_texture_handle)),
        ImageExportSettings {
            output_dir: cfg.output_dir.clone(),
            extension: "png".into(),
        },
    ));
}

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
    let mut min_x = 0.0f32;
    let mut max_x = 0.0f32;
    let mut min_y = 0.0f32;
    let mut max_y = 0.0f32;

    for h in hexes {
        let corners = layout.hex_corners(*h);
        for c in corners {
            min_x = min_x.min(c.x);
            max_x = max_x.max(c.x);
            min_y = min_y.min(c.y);
            max_y = max_y.max(c.y);
        }
    }

    (
        min_x - margin,
        max_x + margin,
        -max_y - margin,
        -min_y + margin,
    )
}

fn exit_after_all_frames(
    mut exit: MessageWriter<AppExit>,
    mut cfg: ResMut<RenderConfig>,
    data: Res<RenderData>,
    loading: Res<LoadingStatus>,
    capturing: Option<Single<&Capturing>>,
) {
    if *loading != LoadingStatus::Loaded || !data.tilings.is_empty() {
        return;
    }

    if cfg.frames_left_to_render == 0 {
        if capturing.is_none() {
            exit.write(AppExit::Success);
        }
    } else {
        cfg.frames_left_to_render -= 1;
    }
}

// Source: Bevy examples <https://bevy.org/examples/games/loading-screen/>
//
// Licensed under MIT license
mod pipelines_ready {
    use bevy::{
        prelude::*,
        render::{render_resource::*, *},
    };

    pub struct PipelinesReadyPlugin;
    impl Plugin for PipelinesReadyPlugin {
        fn build(&self, app: &mut App) {
            app.insert_resource(PipelinesReady::default());

            // In order to gain access to the pipelines status, we have to
            // go into the `RenderApp`, grab the resource from the main App
            // and then update the pipelines status from there.
            // Writing between these Apps can only be done through the
            // `ExtractSchedule`.
            app.sub_app_mut(RenderApp)
                .add_systems(ExtractSchedule, update_pipelines_ready);
        }
    }

    #[derive(Resource, Debug, Default)]
    pub struct PipelinesReady(pub bool);

    fn update_pipelines_ready(mut main_world: ResMut<MainWorld>, pipelines: Res<PipelineCache>) {
        if let Some(mut pipelines_ready) = main_world.get_resource_mut::<PipelinesReady>() {
            pipelines_ready.0 = pipelines.waiting_pipelines().count() == 0;
        }
    }
}
