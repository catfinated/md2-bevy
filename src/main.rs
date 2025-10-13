use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css::*,
    pbr::wireframe::{Wireframe, WireframeColor, WireframeConfig, WireframePlugin},
    prelude::*,
    render::{
        render_resource::{PrimitiveTopology, WgpuFeatures},
        settings::{RenderCreation, WgpuSettings},
        RenderPlugin,
    },
};

use md2_bevy::md2::Mesh as MD2;

fn main() {

    App::new()
        .add_plugins((
            DefaultPlugins.set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            }),
            WireframePlugin::default(),
        ))
        .insert_resource(WireframeConfig {
            global: true,
            default_color: WHITE.into(),
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    //asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mesh_handle: Handle<Mesh> = meshes.add(create_mesh());

    let scale = 1.0_f32 / 32.0_f32;
    let neg90 = f32::to_radians(-90.0);

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(materials.add(Color::from(LIME))),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZEx, 0.0, neg90, neg90)).with_scale(Vec3::splat(scale)),
        Wireframe,
        WireframeColor { color: LIME.into() },
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(1.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((Camera3d::default(), camera_and_light_transform));

    // Light up the scene.
    commands.spawn((PointLight::default(), camera_and_light_transform));
}

fn create_mesh() -> Mesh {

    let mesh = MD2::load(&String::from("data/models/ogro/tris.md2"));
    println!("{:#?}", mesh.header);
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        mesh.key_frames[0].vertices.clone(),
    )
}
