use bevy::{
    asset::{AssetPath, RenderAssetUsages},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use rand::prelude::*;

use md2_bevy::md2::Mesh as MD2;

#[derive(Component)]
struct MD2Component {
    md2: MD2,
    skin_idx: usize,
}

impl MD2Component {
    fn skin_name(&self) -> &str {
        self.md2.skins[self.skin_idx].to_str().unwrap()
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_input_system)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let md2 = MD2::load(&String::from("assets/models/ogro/tris.md2"));
    let skin_idx = rand::rng().random_range(0..md2.skins.len());
    let path = AssetPath::from_path_buf(md2.skins[skin_idx].clone());
    let custom_texture_handle: Handle<Image> = asset_server.load(path);
    let mesh_handle: Handle<Mesh> = meshes.add(create_mesh(&md2));
    let component = MD2Component { md2, skin_idx };
    let skin_name = component.skin_name().to_string();

    let scale = 1.0_f32 / 32.0_f32;
    let neg90 = f32::to_radians(-90.0);

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(custom_texture_handle),
            unlit: true,
            ..default()
        })),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZEx, 0.0, neg90, 0.0))
            .with_scale(Vec3::splat(scale)),
        component,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_transform = Transform::from_xyz(1.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((Camera3d::default(), camera_transform));

    let light_transform = Transform::from_xyz(1.0, 1.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Light up the scene.
    commands.spawn((PointLight::default(), light_transform));

    commands.spawn(Text::new(skin_name));
}

fn create_mesh(md2: &MD2) -> Mesh {
    let frame = 4;
    println!("{:#?}", md2.header);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        md2.key_frames[frame].vertices.clone(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, md2.texcoords.clone())
    //.with_inserted_attribute(
    //    Mesh::ATTRIBUTE_NORMAL,
    //    mesh.key_frames[frame].normals.clone(),
    //)
}

fn keyboard_input_system(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut MD2Component)>,
    mut query2: Query<(Entity, &mut Text)>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyA) {
        for (entity, mut md2c) in query.iter_mut() {
            let new_idx = (md2c.skin_idx + 1) % md2c.md2.skins.len();
            let path = AssetPath::from_path_buf(md2c.md2.skins[new_idx].clone());
            let custom_texture_handle: Handle<Image> = asset_server.load(path);
            let new_mat = MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(custom_texture_handle),
                unlit: true,
                ..default()
            }));
            md2c.skin_idx = new_idx;
            commands.entity(entity).insert(new_mat);

            let name = md2c.skin_name();
            for (e, _) in query2.iter_mut() {
                commands.entity(e).insert(Text::new(name));
            }
        }
    }
}
