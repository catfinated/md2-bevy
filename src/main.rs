use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use md2_bevy::md2::Mesh as MD2;

fn main() {

    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let custom_texture_handle: Handle<Image> = asset_server.load("models/ogro/igdosh.png");
    let mesh_handle: Handle<Mesh> = meshes.add(create_mesh());

    let scale = 1.0_f32 / 32.0_f32;
    let neg90 = f32::to_radians(-90.0);

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(custom_texture_handle),
            unlit: true,
            ..default()
        })),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZEx, 0.0, neg90, 0.0)).with_scale(Vec3::splat(scale)),
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_transform =
        Transform::from_xyz(1.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((Camera3d::default(), camera_transform));

    let light_transform =
        Transform::from_xyz(1.0, 1.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Light up the scene.
    commands.spawn((PointLight::default(), light_transform));
}

fn create_mesh() -> Mesh {

    let frame = 4;
    let mesh = MD2::load(&String::from("assets/models/ogro/tris.md2"));
    println!("{:#?}", mesh.header);
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        mesh.key_frames[frame].vertices.clone(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        mesh.texcoords.clone(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        mesh.key_frames[frame].normals.clone(),
    )
}
